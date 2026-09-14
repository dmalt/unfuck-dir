use std::env;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::PendingUndo;

const STATE_DIRNAME: &str = "unfk";
const UNDO_FNAME: &str = "undo.json";

/// Returns the OS-specific directory for storing unfk's state files.
///
/// # Arguments
///
/// * `os` - Target OS string, e.g. [`std::env::consts::OS`].
pub fn state_dir(os: &str) -> Option<PathBuf> {
    match os {
        "macos" => env::var_os("HOME").map(|x| {
            PathBuf::from(x)
                .join("Library")
                .join("Application Support")
                .join(STATE_DIRNAME)
        }),
        "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            let base = match env::var_os("XDG_STATE_HOME") {
                Some(x) if Path::new(&x).is_absolute() => PathBuf::from(x),
                _ => PathBuf::from(env::var_os("HOME")?).join(".local/state"),
            };
            Some(base.join(STATE_DIRNAME))
        }
        "windows" => env::var_os("LOCALAPPDATA").map(|x| PathBuf::from(x).join(STATE_DIRNAME)),
        _ => None,
    }
}

pub fn save(undo_record: &PendingUndo, state_dir: &Path) -> io::Result<()> {
    let serialized = serde_json::to_string_pretty(undo_record)?;
    fs::create_dir_all(state_dir)?;
    fs::write(state_dir.join(UNDO_FNAME), serialized)?;
    Ok(())
}

pub fn load(state_dir: &Path) -> io::Result<PendingUndo> {
    let fp = state_dir.join(UNDO_FNAME);
    let contents = fs::read_to_string(fp)?;
    let undo_record = serde_json::from_str(&contents)?;
    Ok(undo_record)
}

pub fn clear(state_dir: &Path) -> io::Result<()> {
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
        FileIdentity, Move, CompletedMove,
        temp_env::{with_var, with_vars},
    };

    #[test]
    fn unknown_os_returns_none() {
        assert_eq!(state_dir("manotaur_unicorn"), None);
    }

    #[test]
    fn macos_happy_path() {
        let home = "/Users/test";
        let expected = PathBuf::from(home)
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
        let expected = PathBuf::from(xdg_state_home).join(STATE_DIRNAME);
        with_var("XDG_STATE_HOME", Some(xdg_state_home), || {
            assert_eq!(state_dir("linux"), Some(expected))
        });
    }

    #[test]
    fn bsd_flavors_use_linux_layout() {
        let xdg_state_home = "/wherever/that/is";
        let expected = PathBuf::from(xdg_state_home).join(STATE_DIRNAME);
        for os in ["freebsd", "openbsd", "netbsd", "dragonfly"] {
            with_var("XDG_STATE_HOME", Some(xdg_state_home), || {
                assert_eq!(state_dir(os), Some(expected.clone()))
            });
        }
    }

    #[test]
    fn linux_xdg_not_set_returns_default() {
        let home = "/home/test";
        let expected = PathBuf::from(home).join(".local/state").join(STATE_DIRNAME);
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
        let expected = PathBuf::from(localappdata).join(STATE_DIRNAME);
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

    fn sample_move(src: &str, cat: &str, identity: Option<FileIdentity>) -> CompletedMove {
        let dst = src.replace("src", "dst");
        let mv = Move {
            src: PathBuf::from(src),
            dst: PathBuf::from(dst),
            category: String::from(cat),
        };
        CompletedMove { mv, identity }
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
            folders: vec![PathBuf::from("./Documents")],
        };
        let temp_state_dir = tempdir().unwrap();
        save(&undo_record_orig, temp_state_dir.path()).unwrap();

        let undo_record_loaded = load(temp_state_dir.path()).unwrap();
        assert_eq!(undo_record_orig, undo_record_loaded);
    }

    mod clear {
        use std::fs;
        use tempfile::tempdir;

        use crate::history::{UNDO_FNAME, clear};

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
}
