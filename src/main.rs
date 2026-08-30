use clap::{Parser, ValueEnum};
use std::env::consts;
use std::fmt::Write;
use std::path::PathBuf;
use std::{collections::HashMap, fs, path, process};
use unfk::history::save;
use unfk::{MoveRecord, UndoRecord, UnfkError};

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
    #[arg(short, long, conflicts_with_all = ["path", "by", "include_dotfiles"])]
    show_categories: bool,

    /// Include the dotfiles
    #[arg(short = 'i', long)]
    include_dotfiles: bool, // TODO: think of a better short flag. -i is confusing

    #[arg(long, conflicts_with_all = ["path", "by", "include_dotfiles", "show_categories"])]
    undo: bool,
}

#[derive(Clone, ValueEnum)]
enum GroupMode {
    Type,
    Date,
}

fn report_dry(grouping: &HashMap<String, Vec<path::PathBuf>>) -> String {
    let mut res = String::new();
    let n: usize = grouping.values().map(|v| v.len()).sum();
    writeln!(res, "Would move {n} file(s):").expect("writing to a String cannot fail");
    let mut sorted: Vec<_> = grouping.iter().collect();
    sorted.sort_by_key(|(k, files)| (std::cmp::Reverse(files.len()), *k));

    for (k, files) in sorted.iter() {
        let v = files.len();
        writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
    }
    res
}

fn report_actual(
    moves: &[Result<MoveRecord, UnfkError>],
    folders: &[Result<PathBuf, UnfkError>],
) -> String {
    let mut res = String::new();
    let n = moves.iter().filter(|x| x.is_ok()).count();
    writeln!(res, "Moved {n} file(s):").expect("writing to a String cannot fail");

    let mut counts: HashMap<String, usize> = HashMap::new();
    for rec in moves.iter().filter_map(|m| m.as_ref().ok()) {
        *counts.entry(rec.mv.category.clone()).or_default() += 1;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
    for (k, v) in sorted.iter() {
        writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
    }
    let errors: Vec<&UnfkError> = folders
        .iter()
        .filter_map(|r| r.as_ref().err())
        .chain(moves.iter().filter_map(|r| r.as_ref().err()))
        .collect();
    if !errors.is_empty() {
        writeln!(res, "\nERRORS:").expect("writing to a String cannot fail");
        for e in errors {
            writeln!(res, "{e}").expect("writing to a String cannot fail");
        }
    }
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
    let folders_plan = unfk::plan_folders(&files_grouping, &path);
    let moves_plan = unfk::plan_moves(&files_grouping, &path);

    if args.verbose {
        for f in &folders_plan {
            println!("[mkdir] {f:?}");
        }
        if !folders_plan.is_empty() {
            println!();
        }

        for mv in &moves_plan {
            println!("{}", unfk::format_mv(&mv.src, &mv.dst));
        }
    }

    let stats = if args.dry {
        report_dry(&files_grouping)
    } else {
        let folder_results: Vec<_> = folders_plan.into_iter().map(unfk::create_folder).collect();
        let move_results: Vec<_> = moves_plan.into_iter().map(unfk::perform_move).collect();
        let report = report_actual(&move_results, &folder_results);
        if let Some(state_dir) = unfk::history::state_dir(consts::OS) {
            let undo_rec = UndoRecord {
                moves: move_results.into_iter().filter_map(Result::ok).collect(),
                folders: folder_results.into_iter().filter_map(Result::ok).collect(),
            };
            if let Err(e) = save(&undo_rec, &state_dir) {
                eprintln!(
                    "Failed to write the undo log: {e}. The files were still moved but the --undo operation would be unavailable."
                );
            }
        } else {
            eprintln!(
                "Failed to obtain the state directory. The files were still moved but the --undo operation would be unavailable."
            );
        }
        report
    };

    eprintln!("{stats}");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use unfk::Move;

    use super::*;
    use std::io;

    fn ok_move(src: &str, cat: &str) -> Result<MoveRecord, UnfkError> {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        Ok(MoveRecord { mv, identity: None })
    }

    fn fail_move(src: &str, cat: &str) -> Result<MoveRecord, UnfkError> {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        let source = io::Error::new(io::ErrorKind::NotFound, "test_error");
        Err(UnfkError::Move { mv, source })
    }

    #[test]
    fn report_actual_shows_correct_counts_and_errors() {
        let moves = vec![
            ok_move("/doc1_src.pdf", "Documents"),
            ok_move("/doc2_src.pdf", "Documents"),
            ok_move("/book1_src.epub", "Books"),
            fail_move("/book2_src.avi", "Books"),
            fail_move("/movie1_src.avi", "Movies"),
        ];
        let folders = vec![
            Ok(PathBuf::from("./Documents")),
            Ok(PathBuf::from("./Books")),
            Ok(PathBuf::from("./Movies")),
        ];
        let stats = report_actual(&moves, &folders);
        let mut rows = stats.lines();
        println!("{stats}");
        assert_eq!(rows.next(), Some("Moved 3 file(s):"));
        let cols1: Vec<_> = rows.next().expect("row 1").split_whitespace().collect();
        assert_eq!(cols1, ["Documents", "2"]);
        let cols2: Vec<_> = rows.next().expect("row 2").split_whitespace().collect();
        assert_eq!(cols2, ["Books", "1"]);
        assert_eq!(rows.next(), Some(""));

        assert!(rows.next().unwrap().contains("ERRORS"));
        assert!(rows.next().unwrap().contains("book2_src.avi"));
        assert!(rows.next().unwrap().contains("movie1_src.avi"));
        assert_eq!(rows.next(), None);
    }
}
