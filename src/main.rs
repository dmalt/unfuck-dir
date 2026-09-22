use clap::{Parser, ValueEnum};
use std::env::consts;
use std::fmt::Write;
use std::io;
use std::process::ExitCode;
use std::{collections::HashMap, fs, path, process};
use unfk::RunPlan;
use unfk::undo::PendingUndo;
use unfk::undo::{clear, save};

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

fn report_dry(plan: PendingUndo) -> String {
    let mut res = String::new();
    let n = plan.moves.len();
    writeln!(res, "Would move {n} file(s):").expect("writing to a String cannot fail");

    let mut counts: HashMap<String, usize> = HashMap::new();
    for mv in plan.moves.iter() {
        *counts.entry(mv.mv.category.clone()).or_default() += 1;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
    for (k, v) in sorted.iter() {
        writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
    }
    res
}

/// Reverse the most recent run. Report everything to stderr itself
/// and return the process exit code
fn undo(dry: bool) -> ExitCode {
    let Some(state_dir) = unfk::undo::state_dir(consts::OS) else {
        eprintln!("Could not determine the state directory");
        return ExitCode::FAILURE;
    };
    let undo_plan = match unfk::undo::load(&state_dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            eprintln!("Nothing to undo!");
            return ExitCode::SUCCESS;
        }
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            eprintln!("The undo file is damaged!");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        Ok(undo_plan) => undo_plan,
    };

    // n = total moves
    // plan.check() -> good moves, bad moves + reason
    //
    // if dry: cumulative report; if verbose, report the individual fails and good moves
    // actual_undo_moves(good_moves) -> bad_moves + reason
    // merge bad moves from check and the actual undo
    // if there are any bad moves, overwrite the undo log
    // report the overall thing; if verbose; show the individual moves and fails

    // eprintln!("Would skip {}: {reason}", rec.mv.dst.display());
    // let mut moves_result: Vec<Result<CompletedMove, (CompletedMove, SkipReason)>> = Vec::new();
    // for rec in undo_plan.moves {
    //     match rec.check_undo() {
    //         Err(reason) => moves_result.push(Err((rec, reason))),
    //         Ok(()) => moves_result.push(Ok(rec)),
    //     }
    // }

    if dry {
        if dry {
            print!("{undo_plan}");
        }
        report_dry(undo_plan);
        return ExitCode::SUCCESS;
    }
    let undo_outcome = undo_plan.execute();

    if let Some(undo_rec) = undo_outcome.failed() {
        eprintln!("Some entries could not be reverted and remain queued for the next --undo.");
        if let Err(e) = save(&undo_rec, &state_dir) {
            eprintln!("Failed to write the undo log for the failed undos: {e}.");
            return ExitCode::FAILURE;
        }
    } else if let Err(e) = clear(&state_dir) {
        eprintln!("Everything was reverted, but couldn't remove the undo log: {e}.");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args = Args::parse();

    if args.show_categories {
        println!("{}", unfk::group::format_type_to_exts());
        return ExitCode::SUCCESS;
    }

    if args.dry {
        eprintln!("DRY RUN\n");
    }

    if args.undo {
        return undo(args.dry);
    }

    let Ok(path) = unfk::maybe_expand_tilde(&args.path) else {
        eprintln!(
            "Failed to expand tilde in '{}'. Is the $HOME env var set?",
            &args.path
        );
        return ExitCode::FAILURE;
    };
    let files_grouping = group_files(&path, args.include_dotfiles, args.by);

    let plan = RunPlan::make(&files_grouping, &path);

    if args.dry {
        if args.verbose {
            print!("{plan}");
        }
        eprint!("{}", plan.report());
        return ExitCode::SUCCESS;
    }

    // TODO: refactor the section below
    let outcome = plan.execute();
    // if args.verbose {
    //     print!("{outcome}");
    // }
    eprint!("{}", outcome.report());
    if let Some(state_dir) = unfk::undo::state_dir(consts::OS) {
        let undo_queue = PendingUndo::from(outcome);
        if let Err(e) = save(&undo_queue, &state_dir) {
            eprintln!(
                "Failed to write the undo log: {e}. The files were still moved but the --undo operation would be unavailable."
            );
        }
    } else {
        eprintln!(
            "Failed to obtain the state directory. The files were still moved but the --undo operation would be unavailable."
        );
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use unfk::{Move, RunOutcome};

    use std::io;

    fn ok_move(src: &str, cat: &str) -> Result<Move, unfk::FailedMove> {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        Ok(mv)
    }

    fn fail_move(src: &str, cat: &str) -> Result<Move, unfk::FailedMove> {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        let source = io::Error::new(io::ErrorKind::NotFound, "test_error");
        Err(unfk::FailedMove::new(mv, source))
    }

    #[test]
    fn outcome_report_shows_correct_counts_and_errors() {
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
        let outcome = RunOutcome { moves, folders };
        let stats = outcome.report();
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
