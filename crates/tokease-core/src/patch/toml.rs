//! TOML helpers for Codex `config.toml` on top of `toml_edit`, which keeps
//! comments, whitespace and key order. These are the decor-preserving edit
//! primitives from CC Switch's Codex projection; the Codex-specific patch
//! itself lives in `adapters::codex`.

use std::path::Path;

use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike, Value};

use super::{decode_utf8, line_column, KeyPath, PatchError};

/// Parse the current file. A missing or blank file is the empty document;
/// anything unparsable is an error (we never fall back to an empty document).
pub fn parse(path: &Path, pre: Option<&[u8]>) -> Result<DocumentMut, PatchError> {
    let Some(bytes) = pre else {
        return Ok(DocumentMut::new());
    };
    let text = decode_utf8(path, bytes)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    text.parse::<DocumentMut>().map_err(|err| {
        let (line, column) = err
            .span()
            .map_or((0, 0), |span| line_column(text, span.start));
        PatchError::Parse {
            path: path.to_path_buf(),
            line,
            column,
            message: err.message().to_string(),
        }
    })
}

/// Serialise, keeping a trailing newline (and the BOM if the file had one).
pub fn serialize(doc: &DocumentMut, pre: Option<&[u8]>) -> Vec<u8> {
    let mut text = doc.to_string();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if pre.is_some_and(|b| b.starts_with("\u{feff}".as_bytes())) {
        text.insert(0, '\u{feff}');
    }
    text.into_bytes()
}

/// Set a value in place: an existing value keeps its surrounding whitespace
/// and trailing comment; a missing one is appended.
pub fn put_value(table: &mut dyn TableLike, key: &str, value: Value) {
    match table.get_mut(key) {
        Some(Item::Value(slot)) => {
            let decor = slot.decor().clone();
            *slot = value;
            *slot.decor_mut() = decor;
        }
        Some(slot) => *slot = Item::Value(value),
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Replace or add a sub-table, matching the container's shape: an inline
/// container (or an entry that was inline) gets an inline table; otherwise a
/// standard table that keeps the old table's comments and position.
pub fn put_table(container: &mut dyn TableLike, id: &str, table: Table, container_inline: bool) {
    let existing_inline = matches!(container.get(id), Some(Item::Value(_)));
    if container_inline || existing_inline {
        let inline: InlineTable = table.into_inline_table();
        match container.get_mut(id) {
            Some(slot) => *slot = Item::Value(Value::InlineTable(inline)),
            None => {
                container.insert(id, Item::Value(Value::InlineTable(inline)));
            }
        }
        return;
    }
    let mut table = table;
    match container.get_mut(id) {
        Some(slot) => {
            if let Item::Table(old) = slot {
                *table.decor_mut() = old.decor().clone();
                if let Some(position) = old.position() {
                    table.set_position(Some(position));
                }
            }
            *slot = Item::Table(table);
        }
        None => {
            container.insert(id, Item::Table(table));
        }
    }
}

/// Table-like item at `segments` (standard or inline), if present.
pub fn table_at<'a>(root: &'a Table, segments: &[&str]) -> Option<&'a dyn TableLike> {
    let mut cur: &dyn TableLike = root;
    for seg in segments {
        cur = cur.get(seg)?.as_table_like()?;
    }
    Some(cur)
}

pub fn table_at_mut<'a>(root: &'a mut Table, segments: &[&str]) -> Option<&'a mut dyn TableLike> {
    let mut cur: &mut dyn TableLike = root;
    for seg in segments {
        cur = cur.get_mut(seg)?.as_table_like_mut()?;
    }
    Some(cur)
}

/// Table at `segments`, creating missing (implicit) levels.
pub fn ensure_table_mut<'a>(
    path: &Path,
    root: &'a mut Table,
    segments: &[&str],
) -> Result<&'a mut dyn TableLike, PatchError> {
    let mut cur: &mut dyn TableLike = root;
    for (depth, seg) in segments.iter().enumerate() {
        if !cur.contains_key(seg) {
            let mut t = Table::new();
            t.set_implicit(true);
            cur.insert(seg, Item::Table(t));
        }
        cur = cur
            .get_mut(seg)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| PatchError::Shape {
                path: path.to_path_buf(),
                key_path: KeyPath::new(&segments[..=depth]),
                expected: "a table",
            })?;
    }
    Ok(cur)
}

/// Remove the last key of `segments`, leaving the rest of its table alone.
pub fn remove_nested(root: &mut Table, segments: &[&str]) {
    let Some((last, parents)) = segments.split_last() else {
        return;
    };
    if let Some(table) = table_at_mut(root, parents) {
        table.remove(last);
    }
}

pub fn str_at(root: &Table, segments: &[&str]) -> Option<String> {
    let (last, parents) = segments.split_last()?;
    let value = table_at(root, parents)?.get(last)?.as_str()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_value_keeps_trailing_comment_and_position() {
        let mut doc = parse(Path::new("c"), Some(b"a = 1 # keep\nb = 2\n")).unwrap();
        put_value(doc.as_table_mut(), "a", Value::from("x"));
        put_value(doc.as_table_mut(), "c", Value::from(true));
        assert_eq!(doc.to_string(), "a = \"x\" # keep\nb = 2\nc = true\n");
    }

    #[test]
    fn put_table_matches_container_shape() {
        let mut doc = parse(
            Path::new("c"),
            Some(b"[model_providers.old]\nname = \"o\"\n"),
        )
        .unwrap();
        let mut t = Table::new();
        t.insert("name", Item::Value(Value::from("n")));
        let providers = table_at_mut(doc.as_table_mut(), &["model_providers"]).unwrap();
        put_table(providers, "old", t.clone(), false);
        assert_eq!(doc.to_string(), "[model_providers.old]\nname = \"n\"\n");

        let mut doc = parse(
            Path::new("c"),
            Some(b"model_providers = { old = { name = \"o\" } }\n"),
        )
        .unwrap();
        let providers = table_at_mut(doc.as_table_mut(), &["model_providers"]).unwrap();
        put_table(providers, "new", t, true);
        // toml_edit emits `} , ` between inline entries; only the shape matters.
        let out = doc.to_string().replace(" ,", ",");
        assert_eq!(
            out,
            "model_providers = { old = { name = \"o\" }, new = { name = \"n\" } }\n"
        );
    }

    #[test]
    fn parse_reports_position_and_never_empties_broken_files() {
        let err = parse(Path::new("c"), Some(b"ok = 1\nbroken = \n")).unwrap_err();
        match err {
            PatchError::Parse { line, .. } => assert_eq!(line, 2),
            other => panic!("{other:?}"),
        }
        assert!(parse(Path::new("c"), Some(b"  \n"))
            .unwrap()
            .to_string()
            .is_empty());
    }

    #[test]
    fn remove_nested_leaves_siblings() {
        let mut doc = parse(
            Path::new("c"),
            Some(b"[agents]\ndefault_subagent_model = \"m\"\nother = 1\n"),
        )
        .unwrap();
        remove_nested(doc.as_table_mut(), &["agents", "default_subagent_model"]);
        remove_nested(doc.as_table_mut(), &["memories", "extract_model"]);
        assert_eq!(doc.to_string(), "[agents]\nother = 1\n");
        assert_eq!(str_at(doc.as_table(), &["agents", "other"]), None);
    }
}
