use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path;

use core::fmt::{self, Write};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::Move;
use crate::RunOutcome;

const STATE_DIRNAME: &str = "unfk";
const UNDO_FNAME: &str = "undo.json";

#[derive(Debug)]
pub enum SkipReason {
    Occupied,
    Missing,
    IdentityMismatch,
    MoveFailure(io::Error),
}

#[derive(Debug)]
pub struct FailedUndoMove {
    pub reversal: ReverseMove,
    reason: SkipReason,
}

impl fmt::Display for FailedUndoMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to move {}: {}", self.reversal.mv, self.reason)
    }
}

#[derive(Debug)]
pub struct FailedRmdir {
    pub folder: path::PathBuf,
    reason: io::Error,
}

impl FailedRmdir {
    pub fn new(path: &path::Path, reason: io::Error) -> Self {
        FailedRmdir {
            folder: path.to_path_buf(),
            reason,
        }
    }
}

impl fmt::Display for FailedRmdir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to remove {:?}: {}", self.folder, self.reason)
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Occupied => f.write_str("something is already at the original location"),
            Self::Missing => f.write_str("the file is no longer present"),
            Self::IdentityMismatch => f.write_str("the file has changed since it was moved"),
            Self::MoveFailure(e) => write!(f, "could not move it back: {e}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PendingUndo {
    moves: Vec<ReverseMove>,
    folders: Vec<path::PathBuf>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
struct FileIdentity {
    pub mtime: Option<SystemTime>,
    pub size_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ReverseMove {
    pub mv: Move,
    identity: Option<FileIdentity>,
}

fn identity_matches(stored: &FileIdentity, actual: &FileIdentity) -> bool {
    if stored.size_bytes != actual.size_bytes {
        return false;
    }
    if let Some(smt) = stored.mtime
        && let Some(amt) = actual.mtime
        && smt != amt
    {
        return false;
    }

    true
}

impl ReverseMove {
    pub fn capture(mv: Move) -> Self {
        let identity = read_identity(&mv.dst);
        let mv = mv.flip();
        ReverseMove { mv, identity }
    }

    pub fn execute(self) -> Result<Move, FailedUndoMove> {
        if !self.mv.src.exists() {
            return Err(FailedUndoMove {
                reversal: self,
                reason: SkipReason::Missing,
            });
        }

        if self.mv.dst.exists() {
            // TODO: check if the identity matches and simply move the original
            return Err(FailedUndoMove {
                reversal: self,
                reason: SkipReason::Occupied,
            });
        }

        let src_fi = read_identity(&self.mv.src);
        if let Some(actual_fi) = &src_fi
            && let Some(stored_fi) = &self.identity
            && !identity_matches(stored_fi, actual_fi)
        {
            return Err(FailedUndoMove {
                reversal: self,
                reason: SkipReason::IdentityMismatch,
            });
        }
        match self.mv.clone().execute() {
            Ok(mv) => Ok(mv),
            Err(e) => Err(FailedUndoMove {
                reversal: self,
                reason: SkipReason::MoveFailure(e.reason),
            }),
        }
    }
}

fn read_identity(p: &path::Path) -> Option<FileIdentity> {
    fs::metadata(p).ok().map(|md| FileIdentity {
        mtime: md.modified().ok(),
        size_bytes: md.len(),
    })
}

impl PendingUndo {
    pub fn capture(outcome: RunOutcome) -> Self {
        let mut moves: Vec<ReverseMove> = Vec::new();
        for mv in outcome.moves.into_iter().filter_map(Result::ok) {
            moves.push(ReverseMove::capture(mv));
        }
        PendingUndo {
            moves,
            folders: outcome.folders.into_iter().filter_map(Result::ok).collect(),
        }
    }
}

impl fmt::Display for PendingUndo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for folder in &self.folders {
            writeln!(f, "[rmdir] {folder:?}")?;
        }
        if !self.folders.is_empty() {
            writeln!(f)?;
        }
        for reversal in &self.moves {
            writeln!(f, "{}", reversal.mv)?;
        }
        Ok(())
    }
}

pub enum Removal {
    Success,
    AlreadyGone,
}

struct FolderOutcome {
    pub folder: path::PathBuf,
    removal: Removal,
}

pub struct UndoOutcome {
    pub moves: Vec<Result<Move, FailedUndoMove>>,
    folders: Vec<Result<FolderOutcome, FailedRmdir>>,
}

impl UndoOutcome {
    pub fn failed(&self) -> Option<PendingUndo> {
        let pending_moves: Vec<_> = self
            .moves
            .iter()
            .filter_map(|x| x.as_ref().err())
            .map(|x| x.reversal.clone())
            .collect();
        let pending_folders: Vec<path::PathBuf> = self
            .folders
            .iter()
            .filter_map(|x| x.as_ref().err())
            .map(|x| x.folder.clone())
            .collect();
        if pending_moves.is_empty() && pending_folders.is_empty() {
            return None;
        }
        Some(PendingUndo {
            moves: pending_moves,
            folders: pending_folders,
        })
    }

    pub fn report(&self) -> String {
        let mut res = String::new();
        let n = self.moves.iter().filter(|x| x.is_ok()).count();
        writeln!(res, "Reverted {n} file(s):").expect("writing to a String cannot fail");

        let mut counts: HashMap<String, usize> = HashMap::new();
        for mv in self.moves.iter().filter_map(|m| m.as_ref().ok()) {
            *counts.entry(mv.category.clone()).or_default() += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
        for (k, v) in sorted.iter() {
            writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
        }
        let folder_errors: Vec<_> = self
            .folders
            .iter()
            .filter_map(|r| r.as_ref().err())
            .collect();
        let move_errors: Vec<_> = self.moves.iter().filter_map(|r| r.as_ref().err()).collect();
        if !folder_errors.is_empty() || !move_errors.is_empty() {
            writeln!(res, "\nERRORS:").expect("writing to a String cannot fail");
            for e in folder_errors {
                writeln!(res, "{e}").expect("writing to a String cannot fail");
            }
            for e in move_errors {
                writeln!(res, "{e}").expect("writing to a String cannot fail");
            }
        }
        res
    }
}

impl fmt::Display for UndoOutcome {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for folder_res in &self.folders {
            match folder_res {
                Ok(fo) => match fo.removal {
                    Removal::Success => writeln!(f, "[rmdir] {:?}", fo.folder)?,
                    Removal::AlreadyGone => writeln!(f, "[rmdir] {:?} (already gone)", fo.folder)?,
                },
                Err(e) => writeln!(f, "[rmdir failed] {:?}: {}", e.folder, e.reason)?,
            }
        }
        if !self.folders.is_empty() {
            writeln!(f)?;
        }
        for mv_res in &self.moves {
            match mv_res {
                Ok(mv) => writeln!(f, "{}", mv)?,
                Err(fail) => writeln!(f, "[mv failed] {}: {}", fail.reversal.mv, fail.reason)?,
            }
        }
        Ok(())
    }
}

impl PendingUndo {
    pub fn execute(self) -> UndoOutcome {
        let moves: Vec<_> = self.moves.into_iter().map(|m| m.execute()).collect();
        let mut folders: Vec<Result<FolderOutcome, FailedRmdir>> = Vec::new();
        for folder in self.folders {
            match fs::remove_dir(&folder) {
                Err(reason) if reason.kind() == io::ErrorKind::NotFound => {
                    folders.push(Ok(FolderOutcome {
                        folder,
                        removal: Removal::AlreadyGone,
                    }))
                }
                Ok(()) => folders.push(Ok(FolderOutcome {
                    folder,
                    removal: Removal::Success,
                })),
                Err(e) => folders.push(Err(FailedRmdir::new(&folder, e))),
            }
        }

        UndoOutcome { moves, folders }
    }

    pub fn report(&self) -> String {
        let mut res = String::new();
        let n = self.moves.len();
        writeln!(res, "Would revert {n} file(s):").expect("writing to a String cannot fail");

        let mut counts: HashMap<String, usize> = HashMap::new();
        for mv in self.moves.iter() {
            *counts.entry(mv.mv.category.clone()).or_default() += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then(k1.cmp(k2)));
        for (k, v) in sorted.iter() {
            writeln!(res, "  {k:<20} {v:>3}").expect("writing to a String cannot fail");
        }
        res
    }
}
/// Returns the OS-specific directory for storing unfk's state files.
///
/// # Arguments
///
/// * `os` - Target OS string, e.g. [`std::env::consts::OS`].
pub fn state_dir(os: &str) -> Option<path::PathBuf> {
    match os {
        "macos" => env::var_os("HOME").map(|x| {
            path::PathBuf::from(x)
                .join("Library")
                .join("Application Support")
                .join(STATE_DIRNAME)
        }),
        "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            let base = match env::var_os("XDG_STATE_HOME") {
                Some(x) if path::Path::new(&x).is_absolute() => path::PathBuf::from(x),
                _ => path::PathBuf::from(env::var_os("HOME")?).join(".local/state"),
            };
            Some(base.join(STATE_DIRNAME))
        }
        "windows" => {
            env::var_os("LOCALAPPDATA").map(|x| path::PathBuf::from(x).join(STATE_DIRNAME))
        }
        _ => None,
    }
}

pub fn save(undo_record: &PendingUndo, state_dir: &path::Path) -> io::Result<()> {
    let serialized = serde_json::to_string_pretty(undo_record)?;
    fs::create_dir_all(state_dir)?;
    fs::write(state_dir.join(UNDO_FNAME), serialized)?;
    Ok(())
}

pub fn load(state_dir: &path::Path) -> io::Result<PendingUndo> {
    let fp = state_dir.join(UNDO_FNAME);
    let contents = fs::read_to_string(fp)?;
    let undo_record = serde_json::from_str(&contents)?;
    Ok(undo_record)
}

pub fn clear(state_dir: &path::Path) -> io::Result<()> {
    let undo_file = state_dir.join(UNDO_FNAME);
    match fs::remove_file(undo_file) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use tempfile::tempdir;

    use super::*;

    use crate::{
        Move,
        temp_env::{with_var, with_vars},
    };

    #[test]
    fn unknown_os_returns_none() {
        assert_eq!(state_dir("manotaur_unicorn"), None);
    }

    #[test]
    fn macos_happy_path() {
        let home = "/Users/test";
        let expected = path::PathBuf::from(home)
            .join("Library/Application Support")
            .join(STATE_DIRNAME);
        with_var("HOME", Some(home), || {
            assert_eq!(state_dir("macos"), Some(expected))
        });
    }

    #[test]
    fn macos_returns_none_when_home_env_var_is_unset() {
        with_var("HOME", None, || assert_eq!(state_dir("macos"), None));
    }

    #[test]
    fn linux_happy_path() {
        let xdg_state_home = "/wherever/that/is";
        let expected = path::PathBuf::from(xdg_state_home).join(STATE_DIRNAME);
        with_var("XDG_STATE_HOME", Some(xdg_state_home), || {
            assert_eq!(state_dir("linux"), Some(expected))
        });
    }

    #[test]
    fn bsd_flavors_use_linux_layout() {
        let xdg_state_home = "/wherever/that/is";
        let expected = path::PathBuf::from(xdg_state_home).join(STATE_DIRNAME);
        for os in ["freebsd", "openbsd", "netbsd", "dragonfly"] {
            with_var("XDG_STATE_HOME", Some(xdg_state_home), || {
                assert_eq!(state_dir(os), Some(expected.clone()))
            });
        }
    }

    #[test]
    fn linux_xdg_not_set_returns_default() {
        let home = "/home/test";
        let expected = path::PathBuf::from(home)
            .join(".local/state")
            .join(STATE_DIRNAME);
        with_vars(&[("HOME", Some(home)), ("XDG_STATE_HOME", None)], || {
            assert_eq!(state_dir("linux"), Some(expected))
        });
    }

    #[test]
    fn linux_nothing_set_returns_none() {
        with_vars(&[("HOME", None), ("XDG_STATE_HOME", None)], || {
            assert_eq!(state_dir("linux"), None)
        });
    }

    #[test]
    fn windows_happy_path() {
        let localappdata = "/wherever/that/is";
        let expected = path::PathBuf::from(localappdata).join(STATE_DIRNAME);
        with_var("LOCALAPPDATA", Some(localappdata), || {
            assert_eq!(state_dir("windows"), Some(expected))
        });
    }

    #[test]
    fn windows_localappdata_not_set_returns_none() {
        with_var("LOCALAPPDATA", None, || {
            assert_eq!(state_dir("windows"), None)
        });
    }

    fn sample_move(src: &str, cat: &str, identity: Option<FileIdentity>) -> ReverseMove {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: path::PathBuf::from(src),
            dst: path::PathBuf::from(dst),
            category: String::from(cat),
        };
        ReverseMove { mv, identity }
    }

    #[test]
    fn save_load_roundtrip() {
        let mtime = Some(SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 12_456_789));
        let id1 = Some(FileIdentity {
            mtime,
            size_bytes: 4096,
        });
        let id2 = Some(FileIdentity {
            mtime: None,
            size_bytes: 128,
        });
        let undo_record_orig = PendingUndo {
            moves: vec![
                sample_move("doc1_src.pdf", "Documents", id1),
                sample_move("doc2_src.pdf", "Documents", id2),
                sample_move("doc3_src.pdf", "Documents", None),
            ],
            folders: vec![path::PathBuf::from("./Documents")],
        };
        let temp_state_dir = tempdir().unwrap();
        save(&undo_record_orig, temp_state_dir.path()).unwrap();

        let undo_record_loaded = load(temp_state_dir.path()).unwrap();
        assert_eq!(undo_record_orig, undo_record_loaded);
    }

    mod clear {
        use std::fs;
        use tempfile::tempdir;

        use crate::undo::{UNDO_FNAME, clear};

        #[test]
        fn returns_ok_on_missing() {
            let state_dir = tempdir().unwrap();
            assert!(clear(state_dir.path()).is_ok())
        }

        #[test]
        fn removes_existing_undo_file() {
            let state_dir = tempdir().unwrap();
            let undo_path = state_dir.path().join(UNDO_FNAME);
            fs::write(&undo_path, "{}").unwrap();

            assert!(undo_path.exists());
            clear(state_dir.path()).unwrap();

            assert!(!undo_path.exists());
        }
    }

    mod identity_matches {
        use std::time::{Duration, SystemTime};

        use super::{FileIdentity, identity_matches};

        fn id(size_bytes: u64, mtime: Option<SystemTime>) -> FileIdentity {
            FileIdentity { size_bytes, mtime }
        }

        #[test]
        fn identity_matches_mismatched_sizes_are_rejected() {
            let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
            let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);

            #[rustfmt::skip]
            let cases = [
                ("both missing",         None,     None    ),
                ("stored missing",       None,     Some(t1)),
                ("actual missing",       Some(t1), None    ),
                ("both present, equal",  Some(t1), Some(t1)),
                ("both present, differ", Some(t1), Some(t2)),
            ];

            for (name, stored_mtime, actual_mtime) in cases {
                let stored = id(1234, stored_mtime);
                let actual = id(5678, actual_mtime);
                assert!(!identity_matches(&stored, &actual), "case: {name}");
            }
        }

        #[test]
        fn identity_matches_mtime_comparison_at_equal_sizes() {
            let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
            let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);

            #[rustfmt::skip]
            let cases = [
                ("both missing",         None,     None,     true),
                ("stored missing",       None,     Some(t1), true),
                ("actual missing",       Some(t1), None,     true),
                ("both present, equal",  Some(t1), Some(t1), true),
                ("both present, differ", Some(t1), Some(t2), false),
            ];

            for (name, stored_mtime, actual_mtime, expected) in cases {
                let stored = id(4096, stored_mtime);
                let actual = id(4096, actual_mtime);
                assert_eq!(identity_matches(&stored, &actual), expected, "case: {name}");
            }
        }
    }

    fn setup_moved_file(dir: &path::Path) -> ReverseMove {
        let category = String::from("Documents");
        let src = dir.join("a.pdf");
        fs::write(&src, b"hello world").unwrap();
        let cat_dir = dir.join(&category);
        fs::create_dir(&cat_dir).unwrap();
        let dst = cat_dir.join("a.pdf");
        let mv = Move { src, dst, category };
        ReverseMove::capture(mv.execute().unwrap())
    }

    mod check_undo {
        use tempfile::tempdir;

        use super::{FailedUndoMove, SkipReason, setup_moved_file};
        use std::fs;

        #[test]
        fn happy_path() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            assert!(rec.mv.src.exists());
            assert!(!rec.mv.dst.exists());
            assert!(rec.mv.execute().is_ok());
        }

        #[test]
        fn skips_destination_missing() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            fs::remove_file(&rec.mv.src).unwrap();

            let res = rec.execute();

            assert!(res.is_err());

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::Missing,
                        ..
                    })
                ),
                "expected DestinationMissing, got {res:?}"
            );
        }

        #[test]
        fn skips_existing_source() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.dst, content_before).unwrap();

            let res = rec.execute();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::Occupied,
                        ..
                    })
                ),
                "expected SourceOccupied, got {res:?}"
            );
        }

        #[test]
        fn skips_on_identity_mismatch() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.src, content_before).unwrap();

            let res = rec.execute();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::IdentityMismatch,
                        ..
                    })
                ),
                "expected IdentityMismatch, got {res:?}"
            );
        }

        #[test]
        fn proceeds_when_identity_is_not_recorded() {
            let tmp = tempdir().unwrap();
            let mut rec = setup_moved_file(tmp.path());
            rec.identity = None;
            let content_before = b"goodbye world";
            fs::write(&rec.mv.src, content_before).unwrap();

            assert!(rec.execute().is_ok());
        }
    }

    mod undo_move {
        use std::fs;

        use tempfile::tempdir;

        #[cfg(unix)]
        use super::FailedUndoMove;
        use super::SkipReason;
        use super::setup_moved_file;

        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        #[test]
        fn file_is_restored_and_has_proper_content_on_happy_path() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            assert!(rec.mv.src.exists());
            assert!(!rec.mv.dst.exists());
            rec.clone().execute().unwrap();

            assert!(!rec.mv.src.exists());
            assert!(rec.mv.dst.exists());
            assert_eq!(fs::read(&rec.mv.dst).unwrap(), b"hello world");
        }

        #[test]
        fn nothing_changes_on_failed_check() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());
            let content_before = b"goodbye world";
            fs::write(&rec.mv.dst, content_before).unwrap();

            let res = rec.clone().execute();

            assert!(res.is_err(), "Should produce error, got {res:?}");
            assert!(
                rec.mv.dst.exists(),
                "the destination file shouldn't be moved after the failed undo"
            );
            let cont_after = fs::read(&rec.mv.dst).unwrap();
            assert_eq!(
                cont_after, content_before,
                "the source file should stay unchanged"
            );
        }

        #[cfg(unix)]
        #[test]
        fn produces_move_failure_when_cant_perform_move() {
            let tmp = tempdir().unwrap();
            let rec = setup_moved_file(tmp.path());

            fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o555)).unwrap();
            let res = rec.execute();
            // restore the perms so that Drop can remove the tempdir
            fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o755)).unwrap();

            assert!(
                matches!(
                    res,
                    Err(FailedUndoMove {
                        reason: SkipReason::MoveFailure(_),
                        ..
                    })
                ),
                "Expected 'MoveFailure' got {res:?}"
            );
        }
    }
}
