use clap::{Parser, ValueEnum};
use std::env::consts;
use std::{collections::HashMap, fs, path, process};
use unfk::{MoveRecord, UnfkError};

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

    /// Enable verbose output
    #[arg(short, long, default_value = "false")]
    verbose: bool,

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

fn format_stats_dry(grouping: &HashMap<String, Vec<path::PathBuf>>) -> String {
    let mut res = String::new();
    let total_n_files: usize = grouping.values().map(|v| v.len()).sum();
    let header = format!("Would move {} file(s):\n", total_n_files);
    res.push_str(&header);
    let mut sorted: Vec<_> = grouping.iter().collect();
    sorted.sort_by_key(|(_, files)| std::cmp::Reverse(files.len()));

    let rows: String = sorted
        .iter()
        .map(|(k, v)| format!("  {:<20} {:>3}", k, v.len()))
        .collect::<Vec<_>>()
        .join("\n");
    res.push_str(&rows);
    res
}

fn format_stats_actual(moves: &Vec<Result<MoveRecord, UnfkError>>) -> String {
    let mut res = String::new();
    let total_n_files: usize = moves.iter().filter(|x| x.is_ok()).count();

    let header = format!("Moved {} file(s):\n", total_n_files);

    // todo!("change the files count logic to make use of the actual moves");
    res.push_str(&header);
    let mut counts: HashMap<String, usize> = HashMap::new();

    for rec in moves.iter().filter_map(|m| m.as_ref().ok()) {
        *counts.entry(rec.mv.category.clone()).or_default() += 1;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|(_, cnt)| std::cmp::Reverse(*cnt));
    //
    let rows: String = sorted
        .iter()
        .map(|(k, v)| format!("  {:<20} {:>3}", k, v))
        .collect::<Vec<_>>()
        .join("\n");
    res.push_str(&rows);
    res
}

fn is_dotfile(entry: &fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with(".")
}

fn group_files(
    dir_path: &path::Path,
    include_dotfiles: bool,
    by: GroupMode,
) -> HashMap<String, Vec<path::PathBuf>> {
    let Ok(files) = fs::read_dir(dir_path) else {
        eprintln!("Couldn't read directory {}", dir_path.display());
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
        .filter(|e| include_dotfiles || !is_dotfile(e));

    match by {
        GroupMode::Date => {
            let Ok(grouping) = unfk::group::by_date(files) else {
                eprintln!("Accessing files modification date is not supported on this platform.");
                process::exit(1);
            };
            grouping
        }
        GroupMode::Type => unfk::group::by_type(files),
    }
}

fn main() {
    let args = Args::parse();

    if args.show_categories {
        println!("{}", unfk::group::format_type_to_exts());
        process::exit(0);
    }
    let Ok(path) = unfk::maybe_expand_tilde(&args.path) else {
        eprintln!(
            "Failed to expand tilde in '{}'. Is the $HOME env var set?",
            &args.path
        );
        process::exit(1);
    };
    let files_grouping = group_files(&path, args.include_dotfiles, args.by);
    if args.dry {
        eprintln!("DRY RUN\n");
    }
    let folders_to_create = unfk::plan_folders(&files_grouping, &path);
    let moves = unfk::plan_moves(&files_grouping, &path);

    if args.verbose {
        for f in &folders_to_create {
            println!("[mkdir] {f:?}");
        }
        if !folders_to_create.is_empty() {
            println!();
        }

        for mv in &moves {
            println!("{}", unfk::format_mv(&mv.src, &mv.dst));
        }
    }

    let stats;
    if args.dry {
        stats = format_stats_dry(&files_grouping);
    } else {
        let _state_dir = unfk::history::state_dir(consts::OS);
        let folder_results: Vec<_> = folders_to_create
            .into_iter()
            .map(unfk::create_folder)
            .collect();
        let move_results: Vec<_> = moves.into_iter().map(unfk::perform_move).collect();
        stats = format_stats_actual(&move_results);

        let errors: Vec<&UnfkError> = folder_results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .chain(move_results.iter().filter_map(|r| r.as_ref().err()))
            .collect();
        if !errors.is_empty() {
            eprintln!("\nERRORS WHILE MOVING FILES");
            for e in errors {
                eprintln!("{e}");
            }
        }
    }

    eprintln!("\n{}", stats);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use unfk::Move;

    use super::*;
    use std::io;

    #[test]
    fn test_format_stats_actual() {
        let moves = vec![
            Ok(MoveRecord {
                mv: Move {
                    src: PathBuf::from("/test_src"),
                    dst: PathBuf::from("/test_dst"),
                    category: String::from("test_category"),
                },
                identity: None,
            }),
            Ok(MoveRecord {
                mv: Move {
                    src: PathBuf::from("/test_src_2"),
                    dst: PathBuf::from("/test_dst_2"),
                    category: String::from("test_category_2"),
                },
                identity: None,
            }),
            Err(UnfkError::Move {
                mv: Move {
                    src: PathBuf::from("/test_src_3"),
                    dst: PathBuf::from("/test_dst_3"),
                    category: String::from("test_category_3"),
                },
                source: io::Error::new(io::ErrorKind::NotFound, "test_error"),
            }),
        ];
        let stats = format_stats_actual(&moves);
        let good_moves_cnt = moves.iter().filter(|x| x.is_ok()).count();
        assert!(stats.starts_with(&format!("Moved {} file(s):", good_moves_cnt)));
        println!("{stats}");
    }
}
