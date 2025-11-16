mod group;
mod r#move;
use clap::{Parser, ValueEnum};
use std::{fs, path::PathBuf};

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
    dry: bool,

    /// Show the current file extension groups that are used with by="type"
    #[arg(short, long, default_value = "false")]
    show_categories: bool,
}

#[derive(Clone, ValueEnum)]
enum GroupMode {
    Type,
    Date,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.show_categories {
        println!("{}", group::format_type_to_exts());
        return Ok(());
    }

    let path = expand_tilde(&args.path)?;
    let files = fs::read_dir(&path)?;
    let files_grouping = match args.by {
        GroupMode::Date => group::by_date(files)?,
        GroupMode::Type => group::by_type(files)?,
    };
    r#move::move_grouped_files(files_grouping, path, args.dry)?;
    // println!("{:#?}", files);

    Ok(())
}
