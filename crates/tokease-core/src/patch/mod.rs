//! Order-preserving patchers (ported from CC Switch's `live/patch`).
//!
//! Each format has one patch type implementing [`LivePatch`]: input is the
//! file's current bytes (`None` when it does not exist), output is the new
//! bytes. Only the keys a patch names are touched; everything else — other
//! keys, values, ordering, comments, indentation — stays as it was.
//!
//! A file that cannot be parsed is an error, never an empty document: writing
//! from an empty document would wipe the user's configuration.

pub mod dotenv;
pub mod json;
pub mod toml;

use std::fmt;
use std::path::{Path, PathBuf};

/// Position of a key: the sequence of keys from the root. Keys may contain
/// dots (`model."gpt-4.1"` in TOML), so this is not a dotted string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPath(pub Vec<String>);

impl KeyPath {
    pub fn new(segments: &[&str]) -> Self {
        Self(segments.iter().map(|s| (*s).to_string()).collect())
    }

    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn child(&self, key: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(key.to_string());
        Self(segments)
    }

    pub(crate) fn split_last(&self) -> Option<(&[String], &String)> {
        self.0.split_last().map(|(last, parent)| (parent, last))
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("<root>");
        }
        f.write_str(&self.0.join("."))
    }
}

/// Compute a file's new content in memory.
pub trait LivePatch {
    fn apply(&self, path: &Path, pre: Option<&[u8]>) -> Result<Vec<u8>, PatchError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// The file cannot be parsed. Line/column are 1-based.
    #[error("cannot parse {path} (line {line}, column {column}): {message}; nothing was written")]
    Parse {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    /// Parsable, but the place we need to edit has the wrong shape
    /// (e.g. `env` is a string instead of an object).
    #[error(
        "{key_path} in {path} is not {expected}; nothing was written to protect your configuration"
    )]
    Shape {
        path: PathBuf,
        key_path: KeyPath,
        expected: &'static str,
    },
    /// The active Codex profile overrides the route we would write.
    #[error("the active Codex profile \"{profile}\" sets {key}, so requests would not reach Tokease; remove it from [profiles.{profile}] or change the top-level `profile`")]
    Route { profile: String, key: String },
}

impl PatchError {
    pub fn shape(path: &Path, segments: &[String], expected: &'static str) -> Self {
        PatchError::Shape {
            path: path.to_path_buf(),
            key_path: KeyPath(segments.to_vec()),
            expected,
        }
    }
}

/// Byte offset → (line, column), both 1-based.
pub(crate) fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rfind('\n').map_or(before.chars().count(), |nl| {
        before[nl + 1..].chars().count()
    }) + 1;
    (line, column)
}

pub(crate) fn decode_utf8<'a>(path: &Path, bytes: &'a [u8]) -> Result<&'a str, PatchError> {
    std::str::from_utf8(bytes).map_err(|err| {
        let valid = std::str::from_utf8(&bytes[..err.valid_up_to()]).unwrap_or_default();
        let (line, column) = line_column(valid, err.valid_up_to());
        PatchError::Parse {
            path: path.to_path_buf(),
            line,
            column,
            message: "not UTF-8 text".into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_column_counts_from_one() {
        let text = "ab\ncde\nf";
        assert_eq!(line_column(text, 0), (1, 1));
        assert_eq!(line_column(text, 4), (2, 2));
        assert_eq!(line_column(text, text.len()), (3, 2));
    }
}
