//! Finding CLI binaries and asking them for their version.
//!
//! A desktop app launched from the dock/launcher does not inherit the PATH
//! the user's shell builds in `.zshrc`/`.bashrc` (nvm, volta, homebrew on
//! Apple Silicon, …). Like CC Switch we therefore ask the login shell for
//! its PATH once and search that in addition to our own PATH and a few
//! well-known install locations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use super::home_dir;

/// Find the first of `names` on the (augmented) PATH or in `extra` locations.
pub(crate) fn find_binary(names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for name in names {
        if let Ok(p) = which::which(name) {
            return Some(p);
        }
        if let Some(path) = login_shell_path() {
            if let Ok(p) = which::which_in(name, Some(path), &cwd) {
                return Some(p);
            }
        }
    }
    extra.iter().find(|p| p.is_file()).cloned()
}

/// Directories package managers put global CLIs in when they are not on
/// the GUI's PATH. Resolved lazily; missing ones are simply skipped.
pub(crate) fn common_bin_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let mut dirs = vec![
        home.join(".local/bin"),
        home.join(".npm-global/bin"),
        home.join(".volta/bin"),
        home.join(".bun/bin"),
        home.join(".yarn/bin"),
        home.join("Library/pnpm"),
        home.join(".local/share/pnpm"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("pnpm"));
    }
    dirs
}

/// `<dir>/<name>` candidates (with Windows launcher suffixes) for every
/// common bin dir plus the given absolute paths.
pub(crate) fn candidates(name: &str, absolute: &[&str]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = absolute.iter().map(PathBuf::from).collect();
    for dir in common_bin_dirs() {
        out.push(dir.join(name));
        if cfg!(windows) {
            out.push(dir.join(format!("{name}.cmd")));
            out.push(dir.join(format!("{name}.exe")));
        }
    }
    out
}

/// PATH as the user's login shell sees it (unix only). Computed once per
/// process; `None` when the shell cannot be run or is too slow.
pub(crate) fn login_shell_path() -> Option<&'static str> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(compute_login_shell_path).as_deref()
}

#[cfg(unix)]
fn compute_login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").ok().filter(|s| Path::new(s).is_file()).unwrap_or_else(|| "/bin/sh".into());
    // `/usr/bin/env` rather than `echo $PATH`: correct under fish too, and it
    // bypasses aliases/functions an interactive rc file may define.
    let out = run_with_timeout(Command::new(&shell).arg("-lc").arg("/usr/bin/env"), Duration::from_secs(4))?;
    out.lines().find_map(|l| l.strip_prefix("PATH=")).map(str::to_string)
}

#[cfg(not(unix))]
fn compute_login_shell_path() -> Option<String> {
    None
}

/// `<binary> --version`, first line, trimmed. `None` when the binary does
/// not answer within a couple of seconds. Cached per (path, mtime): node
/// based CLIs take a good fraction of a second to answer and status is
/// refreshed after every UI action.
pub(crate) fn version_of(binary: &Path) -> Option<String> {
    type Cache = Mutex<HashMap<(PathBuf, Option<SystemTime>), Option<String>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (binary.to_path_buf(), std::fs::metadata(binary).and_then(|m| m.modified()).ok());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let version = run_with_timeout(Command::new(binary).arg("--version"), Duration::from_secs(5)).and_then(|out| {
        // Codex prints "codex-cli 0.160.0", Claude "2.1.29 (Claude Code)", Gemini "0.62.0".
        out.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
    });
    if let Ok(mut c) = cache.lock() {
        c.insert(key, version.clone());
    }
    version
}

/// Semantic-ish version triple from a version line like `codex-cli 0.160.0`.
pub fn parse_version(line: &str) -> Option<(u64, u64, u64)> {
    line.split(|c: char| c.is_whitespace() || c == '(' || c == 'v').find_map(|tok| {
        let mut it = tok.trim_matches(|c: char| !c.is_ascii_digit()).split('.');
        let a = it.next()?.parse().ok()?;
        let b = it.next()?.parse().ok()?;
        let c = it.next().and_then(|s| s.split('-').next()).and_then(|s| s.parse().ok()).unwrap_or(0);
        Some((a, b, c))
    })
}

/// Run a command with stdin closed, returning stdout on success within
/// `timeout`; the child is killed otherwise.
fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let deadline = Instant::now() + timeout;
    let mut stdout = child.stdout.take()?;
    // Read on a helper thread so a chatty child cannot block on a full pipe.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
        buf
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let buf = reader.join().ok()?;
                return status.success().then(|| String::from_utf8_lossy(&buf).into_owned());
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_lines() {
        assert_eq!(parse_version("codex-cli 0.160.0"), Some((0, 160, 0)));
        assert_eq!(parse_version("2.1.29 (Claude Code)"), Some((2, 1, 29)));
        assert_eq!(parse_version("0.62.0"), Some((0, 62, 0)));
        assert_eq!(parse_version("v1.2.3-beta"), Some((1, 2, 3)));
        assert_eq!(parse_version("nope"), None);
    }

    #[test]
    fn run_with_timeout_kills_slow_children() {
        #[cfg(unix)]
        {
            assert!(run_with_timeout(Command::new("sleep").arg("5"), Duration::from_millis(100)).is_none());
            assert_eq!(run_with_timeout(Command::new("echo").arg("hi"), Duration::from_secs(2)).as_deref(), Some("hi\n"));
        }
    }
}
