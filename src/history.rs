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
                _ => PathBuf::from(env::var_os("HOME")?)
                    .join(".local")
                    .join("state"),
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
    #[test]
    fn unknown_os_returns_none() {
        assert_eq!(state_dir("manotaur_unicorn"), None);
    }
}
