//! Environment-variable conflict checker, adapted from CC Switch's `env_checker`.
//! Copyright (c) 2025 Jason Young. MIT; the notice is in the repository LICENSE.
//!
//! All three CLIs let environment variables override their config files:
//! `ANTHROPIC_BASE_URL` in `~/.zshrc` silently beats `settings.json`, an
//! exported `OPENAI_API_KEY` is sent to Tokease instead of the token, and so
//! on. We cannot (and must not) edit the user's shell files, but we can tell
//! them where the override comes from.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapters::home_dir;
use crate::redact;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvConflict {
    pub name: String,
    /// `process` for the environment Tokease itself runs in, otherwise the
    /// shell file that sets it.
    pub source: String,
    /// Masked value preview (never the full secret).
    pub preview: String,
}

/// Shell start-up files worth scanning, in a stable order.
pub fn shell_files() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    let home = home_dir();
    [
        ".bashrc",
        ".bash_profile",
        ".profile",
        ".zshrc",
        ".zprofile",
        ".zshenv",
        ".config/fish/config.fish",
    ]
    .iter()
    .map(|f| home.join(f))
    .filter(|p| p.is_file())
    .collect()
}

/// Variables matching any of `patterns` (`FOO_` = prefix, `FOO` = exact)
/// found in the process environment and in the shell files.
pub fn check(patterns: &[&str]) -> Vec<EnvConflict> {
    let mut out = Vec::new();
    for (name, value) in std::env::vars() {
        if matches(&name, patterns) && !value.trim().is_empty() {
            out.push(EnvConflict {
                name,
                source: "process".into(),
                preview: redact::token(&value),
            });
        }
    }
    for file in shell_files() {
        if let Ok(text) = std::fs::read_to_string(&file) {
            out.extend(scan_shell_text(&text, patterns, &file));
        }
    }
    out
}

fn matches(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| {
        if p.ends_with('_') {
            name.starts_with(p)
        } else {
            name == *p
        }
    })
}

/// `export NAME=value`, `NAME=value`, `set -gx NAME value` (fish). Commented
/// lines are skipped. Values are only previewed.
fn scan_shell_text(text: &str, patterns: &[&str], file: &Path) -> Vec<EnvConflict> {
    let source = file.to_string_lossy().into_owned();
    let home = home_dir();
    let source = match file.strip_prefix(&home) {
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => source,
    };
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = if let Some(rest) = line.strip_prefix("set ") {
            // fish: set -gx NAME value
            let mut parts = rest.split_whitespace().skip_while(|p| p.starts_with('-'));
            match (parts.next(), parts.next()) {
                (Some(n), v) => (n.to_string(), v.unwrap_or("").to_string()),
                _ => continue,
            }
        } else {
            let rest = line.strip_prefix("export ").unwrap_or(line);
            let Some((n, v)) = rest.split_once('=') else {
                continue;
            };
            let n = n.trim();
            if n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            (n.to_string(), v.trim().to_string())
        };
        if matches(&name, patterns) {
            let value = value.trim_matches(|c| c == '"' || c == '\'');
            out.push(EnvConflict {
                name,
                source: source.clone(),
                preview: redact::token(value),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_exports_and_fish_sets_but_not_comments() {
        let text = "# export ANTHROPIC_BASE_URL=https://commented\nexport ANTHROPIC_AUTH_TOKEN=\"sk-ant-secret-value\"\nOPENAI_API_KEY=sk-x\nset -gx GEMINI_API_KEY abc\nalias ll='ls -l'\n";
        let found = scan_shell_text(
            text,
            &["ANTHROPIC_", "GEMINI_API_KEY"],
            Path::new("/x/.zshrc"),
        );
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["ANTHROPIC_AUTH_TOKEN", "GEMINI_API_KEY"]);
        assert!(
            !found[0].preview.contains("secret-value"),
            "{}",
            found[0].preview
        );
        assert_eq!(found[0].source, "/x/.zshrc");
    }

    #[test]
    fn exact_and_prefix_patterns() {
        assert!(matches("ANTHROPIC_MODEL", &["ANTHROPIC_"]));
        assert!(!matches("ANTHROPIC", &["ANTHROPIC_"]));
        assert!(matches("OPENAI_API_KEY", &["OPENAI_API_KEY"]));
        assert!(!matches("OPENAI_API_KEY_2", &["OPENAI_API_KEY"]));
    }
}
