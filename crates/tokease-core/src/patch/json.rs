//! JSON patcher (Claude Code `settings.json`, Gemini CLI `settings.json`).
//!
//! `serde_json` runs with `preserve_order`, so keys keep their file order.
//! Under `preserve_order`, `Map::remove` is a swap-remove that would move the
//! last key into the hole, so deletions use `shift_remove`, replacements are
//! in place and new keys are appended to their parent object.
//!
//! Re-serialising keeps the file's indent (spaces or tab), line endings, BOM
//! and trailing newline; escapes and number spellings may be normalised on
//! the first write and are byte-stable afterwards.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use super::{decode_utf8, KeyPath, LivePatch, PatchError};

/// Clear every key in `parent` that matches `is_floor` (provider-owned keys).
#[derive(Clone)]
pub struct ClearScope {
    pub parent: KeyPath,
    pub is_floor: fn(&str) -> bool,
}

#[derive(Clone, Default)]
pub struct JsonPatch {
    /// Cleared first. Keys also present in `set` are kept for in-place update.
    pub clear: Vec<ClearScope>,
    /// Removed by exact path (skipped when `set` has the same path).
    pub remove: Vec<KeyPath>,
    /// Target values: in-place when present, appended otherwise (missing
    /// parent objects are created).
    pub set: Vec<(KeyPath, Value)>,
    /// Written only when missing.
    pub seed: Vec<(KeyPath, Value)>,
    /// Removed only when the current value equals one of the listed values
    /// (outgoing provider-exclusive fields, residue cleanup).
    pub remove_if: Vec<(KeyPath, Vec<Value>)>,
}

impl JsonPatch {
    pub fn apply_to(&self, path: &Path, doc: &mut Value) -> Result<(), PatchError> {
        let targets: HashSet<&KeyPath> = self.set.iter().map(|(k, _)| k).collect();

        for scope in &self.clear {
            let Some(map) = object_at_mut(path, doc, &scope.parent.0)? else { continue };
            let doomed: Vec<String> = map
                .keys()
                .filter(|key| (scope.is_floor)(key) && !targets.contains(&scope.parent.child(key)))
                .cloned()
                .collect();
            for key in doomed {
                map.shift_remove(&key);
            }
        }

        for key_path in &self.remove {
            if targets.contains(key_path) {
                continue;
            }
            let (parent, key) = split(key_path);
            if let Some(map) = object_at_mut(path, doc, parent)? {
                map.shift_remove(key);
            }
        }

        for (key_path, value) in &self.set {
            let (parent, key) = split(key_path);
            let map = ensure_object_mut(path, doc, parent)?;
            match map.get_mut(key) {
                Some(slot) => *slot = value.clone(),
                None => {
                    map.insert(key.clone(), value.clone());
                }
            }
        }

        for (key_path, value) in &self.seed {
            let (parent, key) = split(key_path);
            let map = ensure_object_mut(path, doc, parent)?;
            if !map.contains_key(key) {
                map.insert(key.clone(), value.clone());
            }
        }

        for (key_path, values) in &self.remove_if {
            if targets.contains(key_path) {
                continue;
            }
            let (parent, key) = split(key_path);
            let Some(map) = object_at_mut(path, doc, parent)? else { continue };
            if map.get(key).is_some_and(|cur| values.contains(cur)) {
                map.shift_remove(key);
            }
        }
        Ok(())
    }
}

impl LivePatch for JsonPatch {
    fn apply(&self, path: &Path, pre: Option<&[u8]>) -> Result<Vec<u8>, PatchError> {
        let (mut doc, style) = parse(path, pre)?;
        self.apply_to(path, &mut doc)?;
        serialize(path, &doc, &style)
    }
}

/// Value at `path` in `doc`.
pub fn value_at<'a>(doc: &'a Value, path: &KeyPath) -> Option<&'a Value> {
    path.0.iter().try_fold(doc, |cur, seg| cur.get(seg))
}

/// The file's formatting habits, reused when re-serialising.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonStyle {
    indent: Vec<u8>,
    trailing_newline: bool,
    crlf: bool,
    bom: bool,
}

impl Default for JsonStyle {
    fn default() -> Self {
        Self { indent: b"  ".to_vec(), trailing_newline: true, crlf: false, bom: false }
    }
}

/// Parse the current content. Missing or blank files are the empty object
/// (the only permitted "empty base"); a non-object root is an error.
pub fn parse(path: &Path, pre: Option<&[u8]>) -> Result<(Value, JsonStyle), PatchError> {
    let Some(bytes) = pre else {
        return Ok((Value::Object(Map::new()), JsonStyle::default()));
    };
    let text = decode_utf8(path, bytes)?;
    let (bom, text) = match text.strip_prefix('\u{feff}') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if text.trim().is_empty() {
        return Ok((Value::Object(Map::new()), JsonStyle { bom, ..JsonStyle::default() }));
    }
    let doc: Value = serde_json::from_str(text).map_err(|err| PatchError::Parse {
        path: path.to_path_buf(),
        line: err.line(),
        column: err.column(),
        message: err.to_string(),
    })?;
    if !doc.is_object() {
        return Err(PatchError::shape(path, &[], "an object"));
    }
    let style = JsonStyle {
        indent: detect_indent(text).unwrap_or_else(|| b"  ".to_vec()),
        trailing_newline: text.ends_with('\n'),
        crlf: text.contains("\r\n"),
        bom,
    };
    Ok((doc, style))
}

pub fn serialize(path: &Path, doc: &Value, style: &JsonStyle) -> Result<Vec<u8>, PatchError> {
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&style.indent);
    let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
    doc.serialize(&mut ser).map_err(|err| PatchError::Parse {
        path: path.to_path_buf(),
        line: 0,
        column: 0,
        message: err.to_string(),
    })?;
    if style.trailing_newline {
        out.push(b'\n');
    }
    if style.crlf {
        let mut converted = Vec::with_capacity(out.len() + out.len() / 16);
        for b in out {
            if b == b'\n' {
                converted.push(b'\r');
            }
            converted.push(b);
        }
        out = converted;
    }
    if style.bom {
        let mut with_bom = "\u{feff}".as_bytes().to_vec();
        with_bom.extend_from_slice(&out);
        out = with_bom;
    }
    Ok(out)
}

/// The leading whitespace of the first indented line is one indent level.
/// Mixed spaces/tabs are not recognised: fall back to two spaces.
fn detect_indent(text: &str) -> Option<Vec<u8>> {
    text.lines().skip(1).find_map(|line| {
        let content = line.trim_start();
        if content.is_empty() {
            return None;
        }
        let leading = &line[..line.len() - content.len()];
        let uniform = leading.bytes().all(|b| b == b' ') || leading.bytes().all(|b| b == b'\t');
        (!leading.is_empty() && uniform).then(|| leading.as_bytes().to_vec())
    })
}

fn split(key_path: &KeyPath) -> (&[String], &String) {
    key_path.split_last().expect("patch paths must name a key, not the document root")
}

/// Object at `segments`; `None` when a level is missing, error when a level
/// is not an object.
fn object_at_mut<'a>(
    path: &Path,
    doc: &'a mut Value,
    segments: &[String],
) -> Result<Option<&'a mut Map<String, Value>>, PatchError> {
    let mut cur = doc;
    for (depth, seg) in segments.iter().enumerate() {
        let map = cur.as_object_mut().ok_or_else(|| PatchError::shape(path, &segments[..depth], "an object"))?;
        match map.get_mut(seg) {
            Some(next) => cur = next,
            None => return Ok(None),
        }
    }
    cur.as_object_mut().map(Some).ok_or_else(|| PatchError::shape(path, segments, "an object"))
}

/// Object at `segments`, creating missing levels (appended to their parent).
fn ensure_object_mut<'a>(
    path: &Path,
    doc: &'a mut Value,
    segments: &[String],
) -> Result<&'a mut Map<String, Value>, PatchError> {
    let mut cur = doc;
    for (depth, seg) in segments.iter().enumerate() {
        let map = cur.as_object_mut().ok_or_else(|| PatchError::shape(path, &segments[..depth], "an object"))?;
        cur = map.entry(seg.clone()).or_insert_with(|| Value::Object(Map::new()));
    }
    cur.as_object_mut().ok_or_else(|| PatchError::shape(path, segments, "an object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn apply(patch: &JsonPatch, pre: &str) -> String {
        String::from_utf8(patch.apply(Path::new("settings.json"), Some(pre.as_bytes())).unwrap()).unwrap()
    }

    fn is_anthropic(key: &str) -> bool {
        key.starts_with("ANTHROPIC_")
    }

    #[test]
    fn clears_floor_sets_in_place_and_keeps_order_and_style() {
        let pre = "{\n\t\"theme\": \"dark\",\n\t\"env\": {\n\t\t\"ANTHROPIC_BASE_URL\": \"https://old\",\n\t\t\"FOO\": \"1\",\n\t\t\"ANTHROPIC_MODEL\": \"m\"\n\t}\n}\n";
        let patch = JsonPatch {
            clear: vec![ClearScope { parent: KeyPath::new(&["env"]), is_floor: is_anthropic }],
            set: vec![(KeyPath::new(&["env", "ANTHROPIC_BASE_URL"]), json!("https://new"))],
            ..Default::default()
        };
        let out = apply(&patch, pre);
        assert_eq!(out, "{\n\t\"theme\": \"dark\",\n\t\"env\": {\n\t\t\"ANTHROPIC_BASE_URL\": \"https://new\",\n\t\t\"FOO\": \"1\"\n\t}\n}\n");
    }

    #[test]
    fn refuses_broken_or_non_object_files() {
        let patch = JsonPatch::default();
        assert!(matches!(patch.apply(Path::new("x"), Some(b"{ broken")), Err(PatchError::Parse { .. })));
        assert!(matches!(patch.apply(Path::new("x"), Some(b"[1]")), Err(PatchError::Shape { .. })));
        let patch = JsonPatch { set: vec![(KeyPath::new(&["env", "A"]), json!("1"))], ..Default::default() };
        assert!(matches!(patch.apply(Path::new("x"), Some(br#"{"env":"str"}"#)), Err(PatchError::Shape { .. })));
    }

    #[test]
    fn missing_file_and_remove_if() {
        let patch = JsonPatch {
            set: vec![(KeyPath::new(&["security", "auth", "selectedType"]), json!("gemini-api-key"))],
            ..Default::default()
        };
        let out = patch.apply(Path::new("x"), None).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "{\n  \"security\": {\n    \"auth\": {\n      \"selectedType\": \"gemini-api-key\"\n    }\n  }\n}\n");

        let patch = JsonPatch {
            remove_if: vec![(KeyPath::new(&["env", "W"]), vec![json!("1"), json!(1)])],
            ..Default::default()
        };
        assert_eq!(apply(&patch, r#"{"env":{"W":1,"K":2}}"#), "{\n  \"env\": {\n    \"K\": 2\n  }\n}");
        assert_eq!(apply(&patch, r#"{"env":{"W":"9"}}"#), "{\n  \"env\": {\n    \"W\": \"9\"\n  }\n}");
    }

    #[test]
    fn preserves_crlf_and_bom() {
        let pre = "\u{feff}{\r\n  \"a\": 1\r\n}\r\n";
        let patch = JsonPatch { set: vec![(KeyPath::new(&["b"]), json!(2))], ..Default::default() };
        assert_eq!(apply(&patch, pre), "\u{feff}{\r\n  \"a\": 1,\r\n  \"b\": 2\r\n}\r\n");
    }
}
