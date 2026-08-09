use std::env;
use std::path::{Path, PathBuf};

const STATE_DIRNAME: &str = "unfk";

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

#[cfg(test)]
mod tests {

    use super::*;

    use crate::temp_env::{with_var, with_vars};

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
}
