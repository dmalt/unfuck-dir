mod group;
use clap::{Parser, ValueEnum};
use std::collections::HashMap;
use std::{fs, path::PathBuf};

/// Format the move report
fn format_mv(from: &PathBuf, to: &PathBuf) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let from_short = from.display().to_string().replace(&home, "~");
    let to_short = to.display().to_string().replace(&home, "~");
    format!("{} -> {}", from_short, to_short)
}

/// Move files to folders based on grouping
fn move_files(
    files_grouping: HashMap<String, Vec<PathBuf>>,
    path: PathBuf,
    dry_run: bool,
) -> std::io::Result<()> {
    for (dirname, group_files) in &files_grouping {
        if dirname == "Folders" {
            continue;
        }
        let folder_path = path.join(dirname);

        std::fs::create_dir(&folder_path).ok();
        for file in group_files {
            let fname = file.file_name().unwrap();
            let dst = folder_path.join(&fname);
            if dry_run {
                println!("{}", format_mv(&file, &dst));
            } else {
                std::fs::rename(&file, &dst)?;
            }
        }
    }
    Ok(())
}

/// Expand '~' char to the value of the HOME env variable
fn expand_tilde(path: &str) -> std::io::Result<PathBuf> {
    if path.starts_with("~") {
        let home = std::env::var("HOME")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
        Ok(PathBuf::from(path.replacen("~", &home, 1)))
    } else {
        Ok(PathBuf::from(path))
    }
}

#[derive(Parser)]
#[command(name = "downloads-sorter")]
#[command(about = "Sort files by date or type")]
struct Args {
    /// Path to the folder to organize
    #[arg(short, long, default_value = "~/Downloads")]
    path: String,

    /// Group files by type or date
    #[arg(long, default_value = "type")]
    by: GroupMode,

    /// Report the intended operations without executing them
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

    /// Show the current file extension groups that are used with by="type"
    #[arg(short, long, default_value = "false")]
    show_extension_groups: bool,
}

#[derive(Clone, ValueEnum)]
enum GroupMode {
    Type,
    Date,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.show_extension_groups {
        println!("{}", group::format_type_to_exts());
        return Ok(());
    }

    let path = expand_tilde(&args.path)?;
    let files = fs::read_dir(&path)?;
    let files_grouping = match args.by {
        GroupMode::Date => group::by_date(files)?,
        GroupMode::Type => group::by_type(files)?,
    };
    move_files(files_grouping, path, args.dry_run)?;
    // println!("{:#?}", files);

    Ok(())
}
