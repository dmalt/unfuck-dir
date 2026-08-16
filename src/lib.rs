pub mod group;
pub mod history;

#[cfg(test)]
mod temp_env;

use std::collections::HashMap;
use std::time::SystemTime;
use std::{env, ffi, fs, path};

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

pub fn create_folders(folders: &[path::PathBuf]) -> Vec<Result<(), String>> {
    folders
        .iter()
        .map(|fp| fs::create_dir(fp).map_err(|e| format!("Failed to create {fp:?}: '{e}'")))
        .collect()
}

pub struct Move {
    pub src: path::PathBuf,
    pub dst: path::PathBuf,
}

pub struct FileIdentity {
    pub mtime: Option<SystemTime>,
    pub size_bytes: u64,
}

pub struct MoveRecord {
    pub mv: Move,
    pub identity: Option<FileIdentity>,
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
            });
        }
    }
    moves
}

pub fn perform_moves(moves: Vec<Move>) -> Vec<Result<MoveRecord, String>> {
    let mut res = Vec::new();
    for mv in moves.into_iter() {
        if let Err(e) = fs::rename(&mv.src, &mv.dst) {
            let err_str = format!("Failed to move '{:?}' to '{:?}': '{e}'", mv.src, mv.dst);
            res.push(Err(err_str));
            continue;
        };
        let fi = fs::metadata(&mv.dst).ok().map(|md| FileIdentity {
            mtime: md.modified().ok(),
            size_bytes: md.len(),
        });
        let move_record = MoveRecord { mv, identity: fi };
        res.push(Ok(move_record))
    }
    res
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
}
