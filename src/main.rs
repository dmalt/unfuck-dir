mod group;
mod r#move;
use clap::{Parser, ValueEnum};
use std::{collections::HashMap, fs, path::PathBuf};

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
#[command(about = "Organize files by date or type.")]
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

    /// Just show the current file extension groups that are used with by="type" and exit
    #[arg(short, long)]
    show_categories: bool,

    /// Include the dotfiles
    #[arg(short, long)]
    include_dotfiles: bool,  // TODO: think of a better short flag. -i is confusing
}

#[derive(Clone, ValueEnum)]
enum GroupMode {
    Type,
    Date,
}

fn format_stats(grouping: &HashMap<String, Vec<PathBuf>>) -> String {
    let mut res = String::new();
    let total_n_files: usize = grouping.values().map(|v| v.len()).sum();
    let header = format!("Moved {} files:\n", total_n_files);
    res.push_str(&header);
    let mut sorted: Vec<_> = grouping.iter().collect();
    sorted.sort_by_key(|(_, files)| std::cmp::Reverse(files.len()));
    for (key, value) in sorted {
        let n_files = value.len();
        let row = format!("  {:<20} {:>3}\n", key, n_files);
        res.push_str(&row);
    }
    return res;
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.show_categories {
        println!("{}", group::format_type_to_exts());
        return Ok(());
    }

    let path = expand_tilde(&args.path)?;
    let files = fs::read_dir(&path)?.filter_map(|x| x.ok()).filter(|e| {
        if !args.include_dotfiles {
            !e.file_name().to_string_lossy().starts_with(".")
        } else {
            true
        }
    });

    let files_grouping = match args.by {
        GroupMode::Date => group::by_date(files)?,
        GroupMode::Type => group::by_type(files)?,
    };
    let stats = format_stats(&files_grouping);
    r#move::move_grouped_files(files_grouping, path, args.dry)?;
    println!("\n{}", stats);

    Ok(())
}
