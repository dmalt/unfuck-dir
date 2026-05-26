pub mod group;
pub mod history;

use std::collections::HashMap;
use std::io::ErrorKind::AlreadyExists;
use std::{env, fs, path};

/// Separator used between filename and duplicate number (e.g., "file__1.txt")
const DUPLICATE_DELIMETER: &str = "__";
/// Separator used when showing source and destination paths for the moves
const FORMAT_MOVE_SEPARATOR: &str = " -> ";

/// Format the move report
fn format_mv(from: &path::Path, to: &path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let from_short = from.display().to_string().replace(&home, "~");
    let to_short = to.display().to_string().replace(&home, "~");
    format!("'{}'{FORMAT_MOVE_SEPARATOR}'{}'", from_short, to_short)
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

    path::PathBuf::from(dst.with_file_name(new_name))
}

/// Move files to folders based on grouping
pub fn move_grouped_files(
    files_grouping: HashMap<String, Vec<path::PathBuf>>,
    folder_to_organize: path::PathBuf,
    dry_run: bool,
    verbose: bool,
) -> Vec<String> {
    let mut folder_path;
    let mut errors: Vec<String> = Vec::new();
    for (dirname, group_files) in &files_grouping {
        if dirname == "Folders" {
            continue;
        }
        folder_path = folder_to_organize.join(dirname);

        if !dry_run && let Err(e) = fs::create_dir(&folder_path)
            && e.kind() != AlreadyExists
        {
            errors.push(format!("Failed to create {folder_path:?}: '{e}'"));
            continue;
        };
        for file in group_files {
            let fname = file.file_name().expect("Should be a file path");
            let mut dst = folder_path.join(&fname);
            while dst.exists() {
                dst = rename_duplicate(&dst);
            }
            if dry_run || verbose {
                println!("{}", format_mv(&file, &dst));
            }
            if !dry_run && let Err(e) = fs::rename(&file, &dst) {
                errors.push(format!("Failed to move {file:?}: '{e}'"));
            }
        }
    }
    errors
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
