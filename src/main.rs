use clap::{Parser, ValueEnum};
use std::{collections::HashMap, env::VarError, fs, path::PathBuf, process};
use unfk::{group, move_grouped_files};

/// Expand '~' char to the value of the HOME env variable
fn maybe_expand_tilde(path: &str) -> Result<PathBuf, VarError> {
    if !path.starts_with("~") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(path.replacen("~", &home, 1)))
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
    #[arg(short = 'i', long)]
    include_dotfiles: bool, // TODO: think of a better short flag. -i is confusing
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
    res
}

fn is_dotfile(entry: &fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with(".")
}

fn main() {
    let args = Args::parse();

    if args.show_categories {
        println!("{}", group::format_type_to_exts());
        process::exit(0);
    }

    let Ok(path) = maybe_expand_tilde(&args.path) else {
        eprintln!(
            "Failed to expand tilde in '{}'. Is the $HOME env var set?",
            &args.path
        );
        process::exit(1);
    };
    let Ok(files) = fs::read_dir(&path) else {
        eprintln!("Couldn't read directory {}", path.display());
        process::exit(1);
    };
    let files = files
        .filter_map(|x| match x {
            Ok(e) => Some(e),
            Err(e) => {
                eprintln!("Warning: skipping entry: '{e}'");
                None
            }
        })
        .filter(|e| args.include_dotfiles || !is_dotfile(e));

    let files_grouping = match args.by {
        GroupMode::Date => {
            let Ok(grouping) = group::by_date(files) else {
                eprintln!("Accessing files modification date is not supported on this platform.");
                process::exit(1);
            };
            grouping
        }
        GroupMode::Type => group::by_type(files),
    };
    let stats = format_stats(&files_grouping);
    if args.dry {
        println!("[DRY RUN]");
    }
    if let Err(e) = move_grouped_files(files_grouping, path, args.dry) {
        eprintln!("Couldn't move files: {e}");
        process::exit(1);
    };
    println!("\n{}", stats);
}
