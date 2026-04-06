use std::collections::HashMap;
use std::path::PathBuf;

/// Separator used between filename and duplicate number (e.g., "file__1.txt")
const DUPLICATE_DELIMETER: &str = "__";

/// Format the move report
fn format_mv(from: &PathBuf, to: &PathBuf) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let from_short = from.display().to_string().replace(&home, "~");
    let to_short = to.display().to_string().replace(&home, "~");
    format!("{} -> {}", from_short, to_short)
}

/// Rename a file stem to handle duplicates
fn rename_duplicate_stem(stem: &str) -> String {
    let Some((new_stem, new_sfx)) = stem.rsplit_once(DUPLICATE_DELIMETER) else {
        return format!("{}{}{}", stem, DUPLICATE_DELIMETER, "1");
    };
    let Ok(new_sfx_int) = new_sfx.parse::<u8>() else {
        return format!("{}{}{}", stem, DUPLICATE_DELIMETER, "1");
    };

    let Some(incremented) = new_sfx_int.checked_add(1) else {
        return format!("{}{}{}", stem, DUPLICATE_DELIMETER, "1");
    };

    return format!("{}{}{}", new_stem, DUPLICATE_DELIMETER, incremented);
}

fn rename_duplicate(dst: &PathBuf) -> PathBuf {
    let stem = dst
        .file_prefix()
        .expect("dst should always be a file path")
        .to_str()
        .expect("filename should be a vaild UTF-8");

    let normalized_stem = rename_duplicate_stem(stem);
    let ext = dst.extension().and_then(|x| x.to_str()).unwrap_or("");
    let new_name = format!("{}.{}", normalized_stem, ext);

    PathBuf::from(dst.with_file_name(new_name))
}

/// Move files to folders based on grouping
pub fn move_grouped_files(
    files_grouping: HashMap<String, Vec<PathBuf>>,
    folder_to_organize: PathBuf,
    dry_run: bool,
) -> std::io::Result<()> {
    for (dirname, group_files) in &files_grouping {
        if dirname == "Folders" {
            continue;
        }
        let folder_path = folder_to_organize.join(dirname);

        std::fs::create_dir(&folder_path).ok();
        for file in group_files {
            let fname = file.file_name().expect("Should be a file path");
            let mut dst = folder_path.join(&fname);
            while dst.exists() {
                dst = rename_duplicate(&dst);
            }
            if dry_run {
                println!("{}", format_mv(&file, &dst));
            }
            if !dry_run {
                std::fs::rename(&file, &dst)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename_duplicate_stem_first_duplicate() {
        let res = rename_duplicate_stem("some_name");
        assert_eq!(res, "some_name__1");
    }

    #[test]
    fn test_rename_duplicate_stem_second_duplicate() {
        let res = rename_duplicate_stem("some_name__1");
        assert_eq!(res, "some_name__2");
    }

    #[test]
    fn test_rename_duplicate_stem_nth_duplicate() {
        let res = rename_duplicate_stem("some_name__6");
        assert_eq!(res, "some_name__7");
    }

    #[test]
    fn test_rename_duplicate_stem_max_duplicate() {
        let res = rename_duplicate_stem("some_name__255");
        assert_eq!(res, "some_name__255__1");
    }
}
