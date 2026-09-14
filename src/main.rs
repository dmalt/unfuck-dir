use clap::{Parser, ValueEnum};
use std::env::consts;
use std::fmt::{self, Write};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{collections::HashMap, fs, path, process};
use unfk::history::{clear, save};
use unfk::{CompletedMove, Move, PendingUndo, UnfkError, check_undo, undo_move};

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

/// Reverse the most recent run. Report everything to stderr itself
/// and return the process exit code
fn undo(dry: bool) -> ExitCode {
    let Some(state_dir) = unfk::history::state_dir(consts::OS) else {
        eprintln!("Could not determine the state directory");
        return ExitCode::FAILURE;
    };
    let undo_record = match unfk::history::load(&state_dir) {
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
        Ok(undo_record) => undo_record,
    };

    let mut unresolved_moves: Vec<CompletedMove> = Vec::new();
    let mut unresolved_folders: Vec<PathBuf> = Vec::new();

    if dry {
        for rec in undo_record.moves {
            if let Err(reason) = check_undo(&rec) {
                eprintln!("Would skip {}: {reason}", rec.mv.dst.display());
                unresolved_moves.push(rec);
            }
        }
    } else {
        for rec in undo_record.moves {
            if let Err(reason) = undo_move(&rec) {
                eprintln!("Skipping {}: {reason}", rec.mv.dst.display());
                unresolved_moves.push(rec);
            }
        }

        for folder in undo_record.folders {
            if let Err(reason) = fs::remove_dir(&folder) {
                if reason.kind() == io::ErrorKind::NotFound {
                    eprintln!("{folder:?} was already removed");
                } else {
                    eprintln!("Skipping {folder:?}: {reason}");
                    unresolved_folders.push(folder);
                }
            }
        }

        if !(unresolved_moves.is_empty() && unresolved_folders.is_empty()) {
            eprintln!("Some entries could not be reverted and remain queued for the next --undo.");
            let undo_rec = PendingUndo {
                moves: unresolved_moves,
                folders: unresolved_folders,
            };
            if let Err(e) = save(&undo_rec, &state_dir) {
                eprintln!("Failed to write the undo log for the failed undos: {e}.");
                return ExitCode::FAILURE;
            }
        } else if let Err(e) = clear(&state_dir) {
            eprintln!("Everything was reverted, but couldn't remove the undo log: {e}.");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

struct RunOutcome {
    moves: Vec<Result<CompletedMove, UnfkError>>,
    folders: Vec<Result<PathBuf, UnfkError>>,
}

impl RunOutcome {
    pub fn report(&self) -> String {
        let mut res = String::new();
        let n = self.moves.iter().filter(|x| x.is_ok()).count();
        writeln!(res, "Moved {n} file(s):").expect("writing to a String cannot fail");

        let mut counts: HashMap<String, usize> = HashMap::new();
        for rec in self.moves.iter().filter_map(|m| m.as_ref().ok()) {
            *counts.entry(rec.mv.category.clone()).or_default() += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
        for (k, v) in sorted.iter() {
            writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
        }
        let errors: Vec<&UnfkError> = self
            .folders
            .iter()
            .filter_map(|r| r.as_ref().err())
            .chain(self.moves.iter().filter_map(|r| r.as_ref().err()))
            .collect();
        if !errors.is_empty() {
            writeln!(res, "\nERRORS:").expect("writing to a String cannot fail");
            for e in errors {
                writeln!(res, "{e}").expect("writing to a String cannot fail");
            }
        }
        res
    }

    pub fn undoable(self) -> PendingUndo {
        PendingUndo {
            moves: self.moves.into_iter().filter_map(Result::ok).collect(),
            folders: self.folders.into_iter().filter_map(Result::ok).collect(),
        }
    }
}

struct Plan {
    moves: Vec<Move>,
    folders: Vec<PathBuf>,
}

impl Plan {
    pub fn make(files_grouping: &HashMap<String, Vec<path::PathBuf>>, path: &path::Path) -> Self {
        let folders = unfk::plan_folders(files_grouping, path);
        let moves = unfk::plan_moves(files_grouping, path);
        Plan { folders, moves }
    }

    pub fn execute(self) -> RunOutcome {
        let folder_results: Vec<_> = self.folders.into_iter().map(unfk::create_folder).collect();
        let move_results: Vec<_> = self.moves.into_iter().map(unfk::perform_move).collect();

        RunOutcome {
            moves: move_results,
            folders: folder_results,
        }
    }

    pub fn report(&self) -> String {
        let mut res = String::new();
        let n = self.moves.len();
        writeln!(res, "Would move {n} file(s):").expect("writing to a String cannot fail");

        let mut counts: HashMap<String, usize> = HashMap::new();
        for mv in self.moves.iter() {
            *counts.entry(mv.category.clone()).or_default() += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
        for (k, v) in sorted.iter() {
            writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
        }
        res
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for folder in &self.folders {
            writeln!(f, "[mkdir] {folder:?}")?;
        }
        if !self.folders.is_empty() {
            writeln!(f)?;
        }
        for mv in &self.moves {
            writeln!(f, "{}", unfk::format_mv(&mv.src, &mv.dst))?;
        }
        Ok(())
    }
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

    let plan = Plan::make(&files_grouping, &path);
    if args.verbose {
        print!("{plan}");
    }

    if args.dry {
        eprint!("{}", plan.report());
        return ExitCode::SUCCESS;
    }

    // TODO: refactor the section below
    let outcome = plan.execute();
    eprint!("{}", outcome.report());
    if let Some(state_dir) = unfk::history::state_dir(consts::OS) {
        let undo_queue = outcome.undoable();
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

    use unfk::Move;

    use super::*;
    use std::io;

    fn ok_move(src: &str, cat: &str) -> Result<CompletedMove, UnfkError> {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        Ok(CompletedMove { mv, identity: None })
    }

    fn fail_move(src: &str, cat: &str) -> Result<CompletedMove, UnfkError> {
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
