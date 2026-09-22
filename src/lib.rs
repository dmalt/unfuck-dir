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

/// Plan folders creation to move the files into.
/// Existing destination folders are not added to the plan.
pub fn plan_folders(
    files_grouping: &HashMap<String, Vec<path::PathBuf>>,
    folder_to_organize: &path::Path,
) -> Vec<path::PathBuf> {
    let mut folder_path;
    let mut folders_to_create: Vec<path::PathBuf> = Vec::new();
    for dirname in files_grouping.keys() {
        folder_path = folder_to_organize.join(dirname);

        if !folder_path.exists() {
            folders_to_create.push(folder_path);
        }
    }
    folders_to_create
}

#[derive(Debug)]
pub struct FailedMove {
    mv: Move,
    source: io::Error,
}

impl FailedMove {
    pub fn new(mv: Move, source: io::Error) -> Self {
        FailedMove { mv, source }
    }
}

impl fmt::Display for FailedMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Failed to move {:?} to {:?}: {}",
            self.mv.src, self.mv.dst, self.source
        )
    }
}

#[derive(Debug)]
pub struct FailedMkdir {
    path: path::PathBuf,
    source: io::Error,
}

impl FailedMkdir {
    pub fn new(path: &path::Path, source: io::Error) -> Self {
        FailedMkdir {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for FailedMkdir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to create {:?}: {}", self.path, self.source)
    }
}

pub fn create_folder(folder: path::PathBuf) -> Result<path::PathBuf, FailedMkdir> {
    fs::create_dir(&folder).map_err(|e| FailedMkdir::new(&folder, e))?;
    Ok(folder)
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Move {
    pub src: path::PathBuf,
    pub dst: path::PathBuf,
    pub category: String,
}

impl Move {
    pub fn flip(&self) -> Self {
        Move {
            src: self.dst.clone(),
            dst: self.src.clone(),
            category: self.category.clone(),
        }
    }

    pub fn execute(self) -> Result<CompletedMove, FailedMove> {
        fs::rename(&self.src, &self.dst).map_err(|e| FailedMove::new(self.clone(), e))?;
        let fi = read_identity(&self.dst);
        let move_record = CompletedMove {
            mv: self,
            identity: fi,
        };
        Ok(move_record)
    }

    pub fn undo(self) -> Result<Move, FailedMove> {
        fs::rename(&self.dst, &self.src).map_err(|e| FailedMove::new(self.flip(), e))?;
        Ok(self)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct FileIdentity {
    pub mtime: Option<SystemTime>,
    pub size_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CompletedMove {
    pub mv: Move,
    pub identity: Option<FileIdentity>,
}

#[derive(Debug)]
pub enum SkipReason {
    SourceOccupied,
    DestinationMissing,
    IdentityMismatch,
    MoveFailure(io::Error),
}

#[derive(Debug)]
pub struct FailedUndoMove {
    pub mv: CompletedMove,
    pub reason: SkipReason,
}

#[derive(Debug)]
pub struct FailedRmdir {
    pub path: path::PathBuf,
    source: io::Error,
}

impl FailedRmdir {
    pub fn new(path: &path::Path, source: io::Error) -> Self {
        FailedRmdir {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for FailedRmdir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to remove {:?}: {}", self.path, self.source)
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SourceOccupied => f.write_str("something is already at the original location"),
            Self::DestinationMissing => f.write_str("the file is no longer there"),
            Self::IdentityMismatch => f.write_str("the file has changed since it was moved"),
            Self::MoveFailure(e) => write!(f, "could not move it back: {e}"),
        }
    }
}

impl CompletedMove {
    pub fn undo(&self) -> Result<Move, FailedUndoMove> {
        if !self.mv.dst.exists() {
            return Err(FailedUndoMove {
                mv: self.clone(),
                reason: SkipReason::DestinationMissing,
            });
        }

        if self.mv.src.exists() {
            return Err(FailedUndoMove {
                mv: self.clone(),
                reason: SkipReason::SourceOccupied,
            });
        }

        let dst_fi = read_identity(&self.mv.dst);
        if let Some(actual_fi) = &dst_fi
            && let Some(stored_fi) = &self.identity
            && !identity_matches(stored_fi, actual_fi)
        {
            return Err(FailedUndoMove {
                mv: self.clone(),
                reason: SkipReason::IdentityMismatch,
            });
        }
        if let Err(e) = fs::rename(&self.mv.dst, &self.mv.src) {
            return Err(FailedUndoMove {
                mv: self.clone(),
                reason: SkipReason::MoveFailure(e),
            });
        }
        Ok(self.mv.flip())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PendingUndo {
    pub moves: Vec<CompletedMove>,
    pub folders: Vec<path::PathBuf>,
}

pub enum FolderOutcomeType {
    SuccessfullyRemoved,
    AlreadyGone,
}

pub struct FolderOutcome {
    pub folder: path::PathBuf,
    pub outcome_type: FolderOutcomeType,
}

pub struct UndoOutcome {
    pub moves: Vec<Result<Move, FailedUndoMove>>,
    pub folders: Vec<Result<FolderOutcome, FailedRmdir>>,
}

impl UndoOutcome {
    pub fn failed(&self) -> Option<PendingUndo> {
        let pending_moves: Vec<_> = self
            .moves
            .iter()
            .filter_map(|x| x.as_ref().err())
            .map(|x| x.mv.clone())
            .collect();
        let pending_folders: Vec<path::PathBuf> = self
            .folders
            .iter()
            .filter_map(|x| x.as_ref().err())
            .map(|x| x.path.clone())
            .collect();
        if pending_moves.is_empty() || pending_folders.is_empty() {
            return None;
        }
        Some(PendingUndo {
            moves: pending_moves,
            folders: pending_folders,
        })
    }
}

impl PendingUndo {
    pub fn execute(self) -> UndoOutcome {
        let moves: Vec<_> = self.moves.iter().map(|e| e.undo()).collect();
        let mut folders: Vec<Result<FolderOutcome, FailedRmdir>> = Vec::new();
        for folder in self.folders {
            match fs::remove_dir(&folder) {
                Err(reason) if reason.kind() == io::ErrorKind::NotFound => {
                    folders.push(Ok(FolderOutcome {
                        folder,
                        outcome_type: FolderOutcomeType::AlreadyGone,
                    }))
                }
                Ok(()) => folders.push(Ok(FolderOutcome {
                    folder,
                    outcome_type: FolderOutcomeType::SuccessfullyRemoved,
                })),
                Err(e) => folders.push(Err(FailedRmdir::new(&folder, e))),
            }
        }

        UndoOutcome { moves, folders }
    }
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
    use std::{fs, path::Path};

    use crate::{CompletedMove, Move};

    // use super::*;

    mod rename_duplicate_stem {
        use crate::rename_duplicate_stem;

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
    }

    mod identity_matches {
        use std::time::{Duration, SystemTime};

        use crate::{FileIdentity, identity_matches};

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
    }

    fn setup_moved_file(dir: &Path) -> CompletedMove {
        let category = String::from("Documents");
        let src = dir.join("a.pdf");
        fs::write(&src, b"hello world").unwrap();
        let cat_dir = dir.join(&category);
        fs::create_dir(&cat_dir).unwrap();
        let dst = cat_dir.join("a.pdf");
        let mv = Move { src, dst, category };
        mv.execute().unwrap()
    }

    mod check_undo {
        use tempfile::tempdir;

        use super::setup_moved_file;
        use crate::{FailedUndoMove, SkipReason};
        use std::fs;

        #[test]
        fn happy_path() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            assert!(rec.mv.dst.exists());
            assert!(!rec.mv.src.exists());
            assert!(rec.mv.undo().is_ok());
        }

        #[test]
        fn skips_destination_missing() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            fs::remove_file(&rec.mv.dst).unwrap();

            let res = rec.undo();

            assert!(res.is_err());

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::DestinationMissing,
                        ..
                    })
                ),
                "expected DestinationMissing, got {res:?}"
            );
        }

        #[test]
        fn skips_existing_source() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.src, content_before).unwrap();

            let res = rec.undo();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::SourceOccupied,
                        ..
                    })
                ),
                "expected SourceOccupied, got {res:?}"
            );
        }

        #[test]
        fn skips_on_identity_mismatch() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.dst, content_before).unwrap();

            let res = rec.undo();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::IdentityMismatch,
                        ..
                    })
                ),
                "expected IdentityMismatch, got {res:?}"
            );
        }

        #[test]
        fn proceeds_when_identity_is_not_recorded() {
            let tmp = tempdir().unwrap();
            let mut rec = setup_moved_file(tmp.path());
            rec.identity = None;
            let content_before = b"goodbye world";
            fs::write(&rec.mv.dst, content_before).unwrap();

            assert!(rec.undo().is_ok());
        }
    }

    mod undo_move {
        use std::fs;

        use tempfile::tempdir;

        use super::setup_moved_file;
        #[cfg(unix)]
        use crate::FailedUndoMove;
        use crate::SkipReason;

        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        #[test]
        fn file_is_restored_and_has_proper_content_on_happy_path() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            assert!(rec.mv.dst.exists());
            assert!(!rec.mv.src.exists());
            rec.undo().unwrap();

            assert!(!rec.mv.dst.exists());
            assert!(rec.mv.src.exists());
            assert_eq!(fs::read(&rec.mv.src).unwrap(), b"hello world");
        }

        #[test]
        fn nothing_changes_on_failed_check() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.src, content_before).unwrap();

            let res = rec.undo();

            assert!(res.is_err(), "Should produce error, got {res:?}");
            assert!(
                rec.mv.dst.exists(),
                "the destination file shouldn't be moved after the failed undo"
            );
            let cont_after = fs::read(&rec.mv.src).unwrap();
            assert_eq!(
                cont_after, content_before,
                "the source file should stay unchanged"
            );
        }

        #[cfg(unix)]
        #[test]
        fn produces_move_failure_when_cant_perform_move() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o555)).unwrap();
            let res = rec.undo();
            // restore the perms so that Drop can remove the tempdir
            fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o755)).unwrap();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::MoveFailure(_),
                        ..
                    })
                ),
                "Expected 'MoveFailure' got {res:?}"
            );
        }
    }
}
