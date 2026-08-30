pub mod group;
pub mod history;

#[cfg(test)]
mod temp_env;

use core::fmt;
use std::collections::HashMap;
use std::io;
use std::time::SystemTime;
use std::{env, ffi, fs, path};

use serde::{Deserialize, Serialize};

/// Separator used between filename and duplicate number (e.g., "file__1.txt")
const DUPLICATE_DELIMETER: &str = "__";
/// Separator used when showing source and destination paths for the moves
const FORMAT_MOVE_SEPARATOR: &str = " -> ";

const MAX_DUPLICATES: u16 = 10000;

/// Format the move report
pub fn format_mv(from: &path::Path, to: &path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut from_short = from.display().to_string();
    if from_short.starts_with(&home) {
        from_short = from_short.replacen(&home, "~", 1);
    }
    let mut to_short = to.display().to_string();
    if to_short.starts_with(&home) {
        to_short = to_short.replacen(&home, "~", 1);
    }
    format!("\"{}\"{FORMAT_MOVE_SEPARATOR}\"{}\"", from_short, to_short)
}

fn try_increment_suffix(stem: &str) -> Option<(&str, u8)> {
    let (new_stem, new_sfx) = stem.rsplit_once(DUPLICATE_DELIMETER)?;
    let new_sfx: u8 = new_sfx.parse().ok()?;
    // if the suffix is larger than 255, we consider it part of the name.
    let incremented = new_sfx.checked_add(1)?;
    Some((new_stem, incremented))
}

/// Rename a file stem to handle duplicates
fn rename_duplicate_stem(stem: &str) -> String {
    let (new_stem, suffix) = try_increment_suffix(stem).unwrap_or((stem, 1));
    format!("{}{}{}", new_stem, DUPLICATE_DELIMETER, suffix)
}

fn rename_duplicate(dst: &path::Path) -> path::PathBuf {
    let stem = dst
        .file_prefix()
        .expect("dst should always be a file path")
        .to_str()
        .expect("filename should be a vaild UTF-8");

    let normalized_stem = rename_duplicate_stem(stem);
    let ext = dst.extension().and_then(|x| x.to_str()).unwrap_or("");
    let new_name = format!("{}.{}", normalized_stem, ext);

    dst.with_file_name(new_name)
}

pub fn plan_folders(
    files_grouping: &HashMap<String, Vec<path::PathBuf>>,
    folder_to_organize: &path::Path,
) -> Vec<path::PathBuf> {
    let mut folder_path;
    let mut folders_to_create: Vec<path::PathBuf> = Vec::new();
    for dirname in files_grouping.keys() {
        if dirname == "Folders" {
            continue;
        }
        folder_path = folder_to_organize.join(dirname);

        if !folder_path.exists() {
            folders_to_create.push(folder_path);
        }
    }
    folders_to_create
}

#[derive(Debug)]
pub enum UnfkError {
    Move {
        mv: Move,
        source: io::Error,
    },
    CreateFolder {
        path: path::PathBuf,
        source: io::Error,
    },
}

impl UnfkError {
    fn create_folder_failed(path: &path::Path, source: io::Error) -> Self {
        Self::CreateFolder {
            path: path.to_path_buf(),
            source,
        }
    }

    fn move_failed(mv: &Move, source: io::Error) -> Self {
        Self::Move {
            mv: mv.clone(),
            source,
        }
    }
}

impl fmt::Display for UnfkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Move { mv, source } => {
                write!(f, "Failed to move {:?} to {:?}: {source}", mv.src, mv.dst)
            }
            Self::CreateFolder { path, source } => {
                write!(f, "Failed to create {path:?}: {source}")
            }
        }
    }
}
pub fn create_folder(folder: path::PathBuf) -> Result<path::PathBuf, UnfkError> {
    fs::create_dir(&folder).map_err(|e| UnfkError::create_folder_failed(&folder, e))?;
    Ok(folder)
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Move {
    pub src: path::PathBuf,
    pub dst: path::PathBuf,
    pub category: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct FileIdentity {
    pub mtime: Option<SystemTime>,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MoveRecord {
    pub mv: Move,
    pub identity: Option<FileIdentity>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct UndoRecord {
    pub moves: Vec<MoveRecord>,
    pub folders: Vec<path::PathBuf>,
}

fn make_nonexistent_dst(folder_path: &path::Path, fname: &ffi::OsStr) -> path::PathBuf {
    let mut dst = folder_path.join(fname);
    for _ in 0..MAX_DUPLICATES {
        if !dst.exists() {
            break;
        }
        dst = rename_duplicate(&dst);
    }
    if dst.exists() {
        panic!("exceeded {MAX_DUPLICATES} duplicates for {dst:?}");
    }
    dst
}

pub fn plan_moves(
    files_grouping: &HashMap<String, Vec<path::PathBuf>>,
    folder_to_organize: &path::Path,
) -> Vec<Move> {
    let mut folder_path;
    let mut moves: Vec<Move> = Vec::new();
    for (dirname, group_files) in files_grouping {
        if dirname == "Folders" {
            continue;
        }
        folder_path = folder_to_organize.join(dirname);

        for src in group_files {
            let fname = src
                .file_name()
                .expect("must be a file path by construction");
            let dst = make_nonexistent_dst(&folder_path, fname);
            moves.push(Move {
                src: src.clone(),
                dst,
                category: dirname.clone(),
            });
        }
    }
    moves
}

fn read_identity(p: &path::Path) -> Option<FileIdentity> {
    fs::metadata(p).ok().map(|md| FileIdentity {
        mtime: md.modified().ok(),
        size_bytes: md.len(),
    })
}

pub fn perform_move(mv: Move) -> Result<MoveRecord, UnfkError> {
    fs::rename(&mv.src, &mv.dst).map_err(|e| UnfkError::move_failed(&mv, e))?;
    let fi = read_identity(&mv.dst);
    let move_record = MoveRecord { mv, identity: fi };
    Ok(move_record)
}

#[derive(Debug)]
pub enum SkipReason {
    SourceOccupied,
    DestinationMissing,
    IdentityMismatch,
    MoveFailure(io::Error),
}

fn identity_matches(stored: &FileIdentity, actual: &FileIdentity) -> bool {
    if stored.size_bytes != actual.size_bytes {
        return false;
    }
    if let Some(smt) = stored.mtime
        && let Some(amt) = actual.mtime
        && smt != amt
    {
        return false;
    }

    true
}

pub fn undo_move(rec: &MoveRecord) -> Result<(), SkipReason> {
    if !rec.mv.dst.exists() {
        return Err(SkipReason::DestinationMissing);
    }

    if rec.mv.src.exists() {
        return Err(SkipReason::SourceOccupied);
    }

    let dst_fi = read_identity(&rec.mv.dst);
    if let Some(actual_fi) = &dst_fi
        && let Some(stored_fi) = &rec.identity
        && !identity_matches(stored_fi, actual_fi)
    {
        return Err(SkipReason::IdentityMismatch);
    }

    fs::rename(&rec.mv.dst, &rec.mv.src).map_err(SkipReason::MoveFailure)
}

/// Expand '~' char to the value of the HOME env variable
pub fn maybe_expand_tilde(path: &str) -> Result<path::PathBuf, env::VarError> {
    if !path.starts_with("~") {
        return Ok(path::PathBuf::from(path));
    }
    let home = std::env::var("HOME")?;
    Ok(path::PathBuf::from(path.replacen("~", &home, 1)))
}

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Duration};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn rename_duplicate_stem_first_duplicate() {
        let res = rename_duplicate_stem("some_name");
        assert_eq!(res, "some_name__1");
    }

    #[test]
    fn rename_duplicate_stem_second_duplicate() {
        let res = rename_duplicate_stem("some_name__1");
        assert_eq!(res, "some_name__2");
    }

    #[test]
    fn rename_duplicate_stem_nth_duplicate() {
        let res = rename_duplicate_stem("some_name__6");
        assert_eq!(res, "some_name__7");
    }

    #[test]
    fn rename_duplicate_stem_max_duplicate() {
        let res = rename_duplicate_stem("some_name__255");
        assert_eq!(res, "some_name__255__1");
    }

    fn id(size_bytes: u64, mtime: Option<SystemTime>) -> FileIdentity {
        FileIdentity { size_bytes, mtime }
    }

    #[test]
    fn identity_matches_mismatched_sizes_are_rejected() {
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);

        #[rustfmt::skip]
        let cases = [
            ("both missing",         None,     None    ),
            ("stored missing",       None,     Some(t1)),
            ("actual missing",       Some(t1), None    ),
            ("both present, equal",  Some(t1), Some(t1)),
            ("both present, differ", Some(t1), Some(t2)),
        ];

        for (name, stored_mtime, actual_mtime) in cases {
            let stored = id(1234, stored_mtime);
            let actual = id(5678, actual_mtime);
            assert!(!identity_matches(&stored, &actual), "case: {name}");
        }
    }

    #[test]
    fn identity_matches_mtime_comparison_at_equal_sizes() {
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);

        #[rustfmt::skip]
        let cases = [
            ("both missing",         None,     None,     true),
            ("stored missing",       None,     Some(t1), true),
            ("actual missing",       Some(t1), None,     true),
            ("both present, equal",  Some(t1), Some(t1), true),
            ("both present, differ", Some(t1), Some(t2), false),
        ];

        for (name, stored_mtime, actual_mtime, expected) in cases {
            let stored = id(4096, stored_mtime);
            let actual = id(4096, actual_mtime);
            assert_eq!(identity_matches(&stored, &actual), expected, "case: {name}");
        }
    }

    fn setup_moved_file(dir: &Path) -> MoveRecord {
        let category = String::from("Documents");
        let src = dir.join("a.pdf");
        fs::write(&src, b"hello world").unwrap();
        fs::create_dir(dir.join(&category)).unwrap();
        let dst = dir.join("Documents").join("a.pdf");
        let mv = Move { src, dst, category };
        perform_move(mv).unwrap()
    }

    #[test]
    fn undo_move_happy_path() {
        let tmp = tempdir().unwrap();
        let rec = setup_moved_file(tmp.path());

        assert!(rec.mv.dst.exists());
        assert!(!rec.mv.src.exists());
        undo_move(&rec).unwrap();

        assert!(!rec.mv.dst.exists());
        assert!(rec.mv.src.exists());
        assert_eq!(fs::read(&rec.mv.src).unwrap(), b"hello world");
    }

    // #[test]
    // fn format_mv_shortens_only_first_home_env_occurence() {
    //     let from =
    //     let res = format_mv(from, to);
    // }
}
