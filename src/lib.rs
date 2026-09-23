pub mod group;
pub mod undo;

#[cfg(test)]
mod temp_env;

use core::fmt::{self, Write};
use std::collections::HashMap;
use std::io;
use std::{env, ffi, fs, path};

use serde::{Deserialize, Serialize};

/// Separator used between filename and duplicate number (e.g., "file__1.txt")
const DUPLICATE_DELIMETER: &str = "__";
/// Separator used when showing source and destination paths for the moves
const FORMAT_MOVE_SEPARATOR: &str = " -> ";

const MAX_DUPLICATES: u16 = 10000;

/// Expand '~' char to the value of the HOME env variable
pub fn maybe_expand_tilde(path: &str) -> Result<path::PathBuf, env::VarError> {
    if !path.starts_with("~") {
        return Ok(path::PathBuf::from(path));
    }
    let home = std::env::var("HOME")?;
    Ok(path::PathBuf::from(path.replacen("~", &home, 1)))
}

pub fn display_path(p: &path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut p_short = p.display().to_string();
    if !home.is_empty() && p_short.starts_with(&home) {
        p_short = p_short.replacen(&home, "~", 1);
    }
    format!("\"{}\"", p_short)
}

/// Format the move report
fn format_mv(from: &path::Path, to: &path::Path) -> String {
    let from_short = display_path(from);
    let to_short = display_path(to);
    format!("{}{FORMAT_MOVE_SEPARATOR}{}", from_short, to_short)
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

#[derive(Debug)]
pub struct FailedMove {
    mv: Move,
    reason: io::Error,
}

impl FailedMove {
    pub fn new(mv: Move, reason: io::Error) -> Self {
        FailedMove { mv, reason }
    }
}

impl fmt::Display for FailedMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to move {}: {}", self.mv, self.reason)
    }
}

#[derive(Debug)]
pub struct FailedMkdir {
    folder: path::PathBuf,
    reason: io::Error,
}

impl FailedMkdir {
    pub fn new(path: &path::Path, source: io::Error) -> Self {
        FailedMkdir {
            folder: path.to_path_buf(),
            reason: source,
        }
    }
}

impl fmt::Display for FailedMkdir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let folder = display_path(&self.folder);
        write!(f, "Failed to create {}: {}", folder, self.reason)
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

    pub fn execute(self) -> Result<Move, FailedMove> {
        fs::rename(&self.src, &self.dst).map_err(|e| FailedMove::new(self.clone(), e))?;
        Ok(self)
    }

    pub fn undo(self) -> Result<Move, FailedMove> {
        fs::rename(&self.dst, &self.src).map_err(|e| FailedMove::new(self.flip(), e))?;
        Ok(self)
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", format_mv(&self.src, &self.dst))
    }
}

fn count_table<'a>(header: &str, noun: &str, categories: impl Iterator<Item = &'a str>) -> String {
    let mut total = 0usize;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for category in categories {
        *counts.entry(category).or_default() += 1;
        total += 1;
    }

    let mut res = String::new();
    if total == 0 {
        writeln!(res, "{header} {total} {noun}s.").expect("writing to a String cannot fail");
        return res;
    } else {
        writeln!(res, "{header} {total} {noun}(s):").expect("writing to a String cannot fail");
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
    for (k, v) in sorted.iter() {
        writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
    }
    res
}

pub struct RunOutcome {
    pub moves: Vec<Result<Move, FailedMove>>,
    pub folders: Vec<Result<path::PathBuf, FailedMkdir>>,
}

impl RunOutcome {
    pub fn report(&self) -> String {
        let ok_moves = self.moves.iter().filter_map(|m| m.as_ref().ok());
        let mut res = count_table("Moved", "file", ok_moves.map(|mv| mv.category.as_str()));

        let folder_errors: Vec<_> = self
            .folders
            .iter()
            .filter_map(|r| r.as_ref().err())
            .collect();
        let move_errors: Vec<_> = self.moves.iter().filter_map(|r| r.as_ref().err()).collect();
        if !folder_errors.is_empty() || !move_errors.is_empty() {
            writeln!(res, "\nERRORS:").expect("writing to a String cannot fail");
            for e in folder_errors {
                writeln!(res, "{e}").expect("writing to a String cannot fail");
            }
            for e in move_errors {
                writeln!(res, "{e}").expect("writing to a String cannot fail");
            }
        }
        res
    }
}

impl fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for folder_res in &self.folders {
            match folder_res {
                Ok(folder) => writeln!(f, "[mkdir] {}", display_path(folder))?,
                Err(failed_mkdir) => {
                    let folder = display_path(&failed_mkdir.folder);
                    writeln!(f, "[mkdir] {} FAILED: {}", folder, failed_mkdir.reason)?
                }
            }
        }
        if !self.folders.is_empty() {
            writeln!(f)?;
        }
        for mv_res in &self.moves {
            match mv_res {
                Ok(mv) => writeln!(f, "{}", mv)?,
                Err(fail) => writeln!(f, "[mv] {} FAILED: {}", fail.mv, fail.reason)?,
            }
        }
        Ok(())
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

fn plan_moves(
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

/// Plan folders creation to move the files into.
/// Existing destination folders are not added to the plan.
fn plan_folders(
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

pub struct RunPlan {
    moves: Vec<Move>,
    folders: Vec<path::PathBuf>,
}

impl RunPlan {
    pub fn make(files_grouping: &HashMap<String, Vec<path::PathBuf>>, path: &path::Path) -> Self {
        let folders = plan_folders(files_grouping, path);
        let moves = plan_moves(files_grouping, path);
        RunPlan { folders, moves }
    }

    pub fn execute(self) -> RunOutcome {
        let folder_results: Vec<_> = self.folders.into_iter().map(create_folder).collect();
        let move_results: Vec<_> = self.moves.into_iter().map(|m| m.execute()).collect();

        RunOutcome {
            moves: move_results,
            folders: folder_results,
        }
    }

    pub fn report(&self) -> String {
        let categories = self.moves.iter().map(|mv| mv.category.as_str());
        count_table("Would move", "file", categories)
    }
}

impl fmt::Display for RunPlan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for folder in &self.folders {
            writeln!(f, "[mkdir] {}", display_path(folder))?;
        }
        if !self.folders.is_empty() {
            writeln!(f)?;
        }
        for mv in &self.moves {
            writeln!(f, "{}", mv)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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

    mod display_path {
        use crate::display_path;
        use crate::temp_env::with_var;
        use std::path::Path;

        #[test]
        fn path_unchanged_when_home_is_unset() {
            with_var("HOME", None, || {
                assert_eq!(display_path(Path::new("/etc/passwd")), "\"/etc/passwd\"");
            });
        }

        #[test]
        fn home_prefix_with_tilde() {
            with_var("HOME", Some("/Users/test"), || {
                let p = Path::new("/Users/test/Downloads/a.pdf");
                assert_eq!(display_path(p), "\"~/Downloads/a.pdf\"");
            });
        }

        #[test]
        fn paths_outside_home_unchanged() {
            with_var("HOME", Some("/Users/test"), || {
                assert_eq!(display_path(Path::new("/etc/passwd")), "\"/etc/passwd\"");
            });
        }

        #[test]
        fn only_the_leading_home_occurrence() {
            with_var("HOME", Some("/Users/test"), || {
                let p = Path::new("/Users/test/backup/Users/test/a.pdf");
                assert_eq!(display_path(p), "\"~/backup/Users/test/a.pdf\"");
            });
        }
    }
}
