//! `.env` patcher (Gemini CLI's `~/.gemini/.env`): line based; comments,
//! blank lines, unrecognised lines and the order of other variables are
//! preserved.

use std::collections::HashSet;
use std::path::Path;

use super::{decode_utf8, LivePatch, PatchError};

#[derive(Clone, Default)]
pub struct DotenvPatch {
    /// Variables matching the predicate are removed first (provider-owned
    /// keys); those also in `set` are kept for in-place update.
    pub clear: Option<fn(&str) -> bool>,
    /// Target values: the first occurrence is rewritten in place (keeping an
    /// `export ` prefix), duplicates are dropped, missing keys are appended.
    pub set: Vec<(String, String)>,
    /// Removed only when the (unquoted) value equals one of the listed values.
    pub remove_if: Vec<(String, Vec<String>)>,
    /// Removed by name (all occurrences). Skipped when `set` has the key.
    pub remove: Vec<String>,
}

#[derive(Debug, Clone)]
struct Line {
    raw: String,
    key: Option<String>,
}

impl LivePatch for DotenvPatch {
    fn apply(&self, path: &Path, pre: Option<&[u8]>) -> Result<Vec<u8>, PatchError> {
        let text = match pre {
            Some(bytes) => decode_utf8(path, bytes)?,
            None => "",
        };
        let crlf = text.contains("\r\n");
        let trailing_newline = pre.is_none() || text.is_empty() || text.ends_with('\n');
        let mut lines: Vec<Line> = if text.is_empty() {
            Vec::new()
        } else {
            text.strip_suffix('\n')
                .unwrap_or(text)
                .split('\n')
                .map(|raw| {
                    let raw = raw.strip_suffix('\r').unwrap_or(raw).to_string();
                    let key = parse_key(&raw).map(str::to_string);
                    Line { raw, key }
                })
                .collect()
        };

        let targets: HashSet<&str> = self.set.iter().map(|(k, _)| k.as_str()).collect();

        if let Some(is_floor) = self.clear {
            lines.retain(|l| l.key.as_deref().is_none_or(|k| !is_floor(k) || targets.contains(k)));
        }

        for (key, value) in &self.set {
            let mut seen = false;
            lines.retain_mut(|l| {
                if l.key.as_deref() != Some(key.as_str()) {
                    return true;
                }
                if seen {
                    return false;
                }
                seen = true;
                let export = if l.raw.trim_start().starts_with("export ") { "export " } else { "" };
                l.raw = format!("{export}{key}={value}");
                true
            });
            if !seen {
                lines.push(Line { raw: format!("{key}={value}"), key: Some(key.clone()) });
            }
        }

        lines.retain(|l| {
            l.key.as_deref().is_none_or(|k| targets.contains(k) || !self.remove.iter().any(|d| d == k))
        });

        for (key, values) in &self.remove_if {
            if targets.contains(key.as_str()) {
                continue;
            }
            lines.retain(|l| {
                l.key.as_deref() != Some(key.as_str()) || !values.iter().any(|v| unquote(value_of(&l.raw)) == v)
            });
        }

        let sep = if crlf { "\r\n" } else { "\n" };
        let mut out = lines.iter().map(|l| l.raw.as_str()).collect::<Vec<_>>().join(sep);
        if trailing_newline && !lines.is_empty() {
            out.push_str(sep);
        }
        Ok(out.into_bytes())
    }
}

/// Variables and (unquoted) values in first-seen order; a repeated key takes
/// its last value, like dotenv parsers do.
pub fn entries(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let Some(key) = parse_key(raw) else { continue };
        let value = unquote(value_of(raw)).to_string();
        match out.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => out.push((key.to_string(), value)),
        }
    }
    out
}

pub fn get(text: &str, key: &str) -> Option<String> {
    entries(text).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Variable name of `KEY=...` / `export KEY=...`; `None` for anything else.
fn parse_key(raw: &str) -> Option<&str> {
    let line = raw.trim_start();
    if line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(key)
}

fn value_of(raw: &str) -> &str {
    raw.split_once('=').map_or("", |(_, v)| v.trim())
}

fn unquote(value: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(q).and_then(|r| r.strip_suffix(q)) {
            return inner;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(patch: &DotenvPatch, pre: Option<&str>) -> String {
        String::from_utf8(patch.apply(Path::new(".env"), pre.map(str::as_bytes)).unwrap()).unwrap()
    }

    fn is_google(key: &str) -> bool {
        key.starts_with("GOOGLE_") || key == "GEMINI_API_KEY"
    }

    #[test]
    fn clears_floor_rewrites_in_place_and_keeps_other_lines() {
        let patch = DotenvPatch {
            clear: Some(is_google),
            set: vec![("GEMINI_API_KEY".into(), "new".into()), ("GEMINI_MODEL".into(), "m".into())],
            ..Default::default()
        };
        let pre = "# mine\nGEMINI_SANDBOX=docker\nexport GEMINI_API_KEY=\"old\"\nGOOGLE_GEMINI_BASE_URL=https://a\nGEMINI_API_KEY=dup\nDEBUG=1\n";
        assert_eq!(
            apply(&patch, Some(pre)),
            "# mine\nGEMINI_SANDBOX=docker\nexport GEMINI_API_KEY=new\nDEBUG=1\nGEMINI_MODEL=m\n"
        );
        assert_eq!(apply(&patch, None), "GEMINI_API_KEY=new\nGEMINI_MODEL=m\n");
    }

    #[test]
    fn entries_take_the_last_value_and_unquote() {
        let text = "A='x'\nB=\"y\"\nA=z\n# C=1\n";
        assert_eq!(entries(text), vec![("A".into(), "z".into()), ("B".into(), "y".into())]);
        assert_eq!(get(text, "B").as_deref(), Some("y"));
    }

    #[test]
    fn crlf_is_preserved() {
        let patch = DotenvPatch { set: vec![("K".into(), "v".into())], ..Default::default() };
        assert_eq!(apply(&patch, Some("A=1\r\n")), "A=1\r\nK=v\r\n");
    }
}
