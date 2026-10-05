//! Local conversation history for the coding tools Tokease connects.
//!
//! Each tool keeps its own transcripts. This module only reads them:
//!
//! * Claude CLI — `~/.claude/projects/*/sessions-index.json`, then the
//!   session jsonl named there (and only if it stays inside that projects dir)
//! * Codex — `$CODEX_HOME/sessions/**/*.jsonl` and `archived_sessions/`
//! * OpenCode — `$XDG_DATA_HOME/opencode/opencode.db` (`session_v2`)
//! * Gemini CLI — `~/.gemini/tmp/**/chats/*.json` when that tree exists
//!
//! Codex transcripts come from `thread_history_1.sqlite` when it exists, so a
//! long rollout file is not cut off after the first few megabytes. Listing
//! still peeks at rollout files for the working directory and model.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::adapters::{home_dir, ClientId};

const LIST_HEAD: usize = 96 * 1024;
const MAX_MESSAGES: usize = 4_000;
const MAX_TEXT: usize = 100_000;
const MAX_SESSIONS: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub client: ClientId,
    pub title: String,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub updated_at: Option<String>,
    pub message_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// `user` or `assistant`.
    pub role: String,
    pub text: String,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTranscript {
    pub session: ChatSession,
    pub messages: Vec<ChatMessage>,
    pub truncated: bool,
    pub missing_file: bool,
}

#[derive(Debug, Clone)]
pub struct HistoryRoots {
    pub claude_dir: PathBuf,
    pub codex_dir: PathBuf,
    pub gemini_dir: PathBuf,
    pub opencode_db: PathBuf,
}

impl HistoryRoots {
    pub fn detect() -> Self {
        let home = home_dir();
        let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".claude"));
        let codex = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".codex"));
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".local/share"));
        Self {
            claude_dir: claude,
            codex_dir: codex,
            gemini_dir: home.join(".gemini"),
            opencode_db: data.join("opencode").join("opencode.db"),
        }
    }
}

struct Found {
    session: ChatSession,
    /// Where the transcript lives. `None` when the index entry's file is gone
    /// or the source is the OpenCode database.
    path: Option<PathBuf>,
    kind: Source,
}

#[derive(Clone, Copy)]
enum Source {
    ClaudeJsonl,
    CodexJsonl,
    GeminiJson,
    OpenCodeDb,
}

pub fn list(roots: &HistoryRoots) -> Vec<ChatSession> {
    let mut found = collect(roots);
    found.sort_by(|a, b| b.session.updated_at.cmp(&a.session.updated_at));
    found.truncate(MAX_SESSIONS);
    found.into_iter().map(|f| f.session).collect()
}

pub fn read(roots: &HistoryRoots, id: &str) -> Option<ChatTranscript> {
    let found = collect(roots).into_iter().find(|f| f.session.id == id)?;
    Some(load_transcript(roots, found))
}

fn collect(roots: &HistoryRoots) -> Vec<Found> {
    let mut out = Vec::new();
    out.extend(claude_sessions(&roots.claude_dir));
    out.extend(codex_sessions(&roots.codex_dir));
    out.extend(gemini_sessions(&roots.gemini_dir));
    match opencode_sessions(&roots.opencode_db) {
        Ok(rows) => out.extend(rows),
        Err(err) => log::warn!("opencode history: {err}"),
    }
    out
}

fn claude_sessions(dir: &Path) -> Vec<Found> {
    let projects = dir.join("projects");
    let mut out = Vec::new();
    for index in collect_files(&projects, 3, "sessions-index.json") {
        let Ok(text) = fs::read_to_string(&index) else {
            continue;
        };
        let Ok(doc) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(entries) = doc.get("entries").and_then(Value::as_array) else {
            continue;
        };
        for entry in entries {
            let Some(sid) = entry.get("sessionId").and_then(Value::as_str) else {
                continue;
            };
            if entry.get("isSidechain").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let full = entry.get("fullPath").and_then(Value::as_str).map(PathBuf::from);
            let path = full.filter(|p| file_within(&projects, p));
            let title = entry
                .get("summary")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    entry
                        .get("firstPrompt")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                })
                .map(title_from)
                .unwrap_or_else(|| "未命名会话".into());
            let updated = entry
                .get("modified")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| path.as_ref().and_then(|p| rfc3339_mtime(p)));
            out.push(Found {
                session: ChatSession {
                    id: format!("claude:{sid}"),
                    client: ClientId::Claude,
                    title,
                    model: None,
                    cwd: entry
                        .get("projectPath")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    updated_at: updated,
                    message_count: entry.get("messageCount").and_then(Value::as_u64).map(|n| n as u32),
                },
                path,
                kind: Source::ClaudeJsonl,
            });
        }
    }
    out
}

fn codex_sessions(dir: &Path) -> Vec<Found> {
    let mut files = collect_files(&dir.join("sessions"), 6, ".jsonl");
    files.extend(collect_files(&dir.join("archived_sessions"), 6, ".jsonl"));
    let mut by_id: BTreeMap<String, Found> = BTreeMap::new();
    for path in files {
        if let Some(found) = codex_head(&path) {
            by_id.insert(found.session.id.clone(), found);
        }
    }
    match codex_db_summaries(&dir.join("thread_history_1.sqlite")) {
        Ok(rows) => {
            for row in rows {
                match by_id.get_mut(&row.session.id) {
                    Some(existing) => {
                        existing.session.message_count = row.session.message_count;
                        if row.session.updated_at.is_some() {
                            existing.session.updated_at = row.session.updated_at;
                        }
                        if existing.session.title == "未命名会话" {
                            existing.session.title = row.session.title;
                        }
                    }
                    None => {
                        by_id.insert(row.session.id.clone(), row);
                    }
                }
            }
        }
        Err(err) => log::warn!("codex history db: {err}"),
    }
    for (id, name) in codex_index_names(&dir.join("session_index.jsonl")) {
        let key = format!("codex:{id}");
        if let Some(found) = by_id.get_mut(&key) {
            if !name.trim().is_empty() {
                found.session.title = title_from(&name);
            }
        }
    }
    by_id.into_values().collect()
}

fn codex_head(path: &Path) -> Option<Found> {
    let head = read_head(path, LIST_HEAD).ok()?;
    let text = String::from_utf8_lossy(&head);
    let mut session_id = None;
    let mut cwd = None;
    let mut model = None;
    let mut title = None;
    let mut updated = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                if let Some(p) = v.get("payload") {
                    session_id = p
                        .get("session_id")
                        .or_else(|| p.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    cwd = p.get("cwd").and_then(Value::as_str).map(str::to_string);
                    updated = p
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            Some("turn_context") => {
                if model.is_none() {
                    model = v
                        .get("payload")
                        .and_then(|p| p.get("model"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            Some("response_item") => {
                if title.is_some() {
                    continue;
                }
                let payload = v.get("payload")?;
                if payload.get("type").and_then(Value::as_str) != Some("message") {
                    continue;
                }
                if payload.get("role").and_then(Value::as_str) != Some("user") {
                    continue;
                }
                let text = text_of(payload.get("content").unwrap_or(&Value::Null));
                if !text.trim().is_empty() {
                    title = Some(title_from(&text));
                }
            }
            _ => {}
        }
    }
    let sid = session_id.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session")
            .to_string()
    });
    Some(Found {
        session: ChatSession {
            id: format!("codex:{sid}"),
            client: ClientId::Codex,
            title: title.unwrap_or_else(|| "未命名会话".into()),
            model,
            cwd,
            updated_at: updated.or_else(|| rfc3339_mtime(path)),
            message_count: None,
        },
        path: Some(path.to_path_buf()),
        kind: Source::CodexJsonl,
    })
}

fn gemini_sessions(dir: &Path) -> Vec<Found> {
    collect_files(&dir.join("tmp"), 5, ".json")
        .into_iter()
        .filter(|p| {
            p.parent()
                .and_then(|d| d.file_name())
                .is_some_and(|n| n == "chats")
        })
        .filter_map(|path| {
            let len = path.metadata().ok()?.len();
            if len > 2_000_000 {
                return None;
            }
            let bytes = fs::read(&path).ok()?;
            let doc: Value = serde_json::from_slice(&bytes).ok()?;
            let sid = doc
                .get("sessionId")
                .and_then(Value::as_str)
                .unwrap_or("session");
            let messages = doc.get("messages").and_then(Value::as_array)?;
            let title = messages.iter().find_map(|m| {
                let role = m.get("role").or_else(|| m.get("type")).and_then(Value::as_str)?;
                if role != "user" {
                    return None;
                }
                let text = message_text(m);
                if text.trim().is_empty() {
                    None
                } else {
                    Some(title_from(&text))
                }
            });
            Some(Found {
                session: ChatSession {
                    id: format!("gemini:{sid}"),
                    client: ClientId::Gemini,
                    title: title.unwrap_or_else(|| "未命名会话".into()),
                    model: doc
                        .get("model")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    cwd: None,
                    updated_at: doc
                        .get("lastUpdated")
                        .or_else(|| doc.get("startTime"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| rfc3339_mtime(&path)),
                    message_count: Some(messages.len() as u32),
                },
                path: Some(path),
                kind: Source::GeminiJson,
            })
        })
        .collect()
}

fn opencode_sessions(db_path: &Path) -> Result<Vec<Found>, String> {
    if !db_path.is_file() {
        return Ok(Vec::new());
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, directory, model, time_updated, time_created
             FROM session_v2
             ORDER BY COALESCE(time_updated, time_created) DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, directory, model, updated, created) = row.map_err(|e| e.to_string())?;
        let when = updated.or(created);
        out.push(Found {
            session: ChatSession {
                id: format!("opencode:{id}"),
                client: ClientId::OpenCode,
                title: title
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| title_from(&s))
                    .unwrap_or_else(|| "未命名会话".into()),
                model: model.as_deref().and_then(model_id_of),
                cwd: directory,
                updated_at: when.and_then(epoch_to_rfc3339),
                message_count: None,
            },
            path: None,
            kind: Source::OpenCodeDb,
        });
    }
    Ok(out)
}

fn load_transcript(roots: &HistoryRoots, found: Found) -> ChatTranscript {
    let (messages, truncated, missing_file) = match found.kind {
        Source::CodexJsonl => codex_transcript(&roots.codex_dir, &found),
        Source::ClaudeJsonl => match &found.path {
            Some(path) if path.is_file() => {
                let (msgs, cut) = jsonl_messages(path, found.kind);
                (msgs, cut, false)
            }
            _ => (Vec::new(), false, true),
        },
        Source::GeminiJson => match &found.path {
            Some(path) => match fs::read(path) {
                Ok(bytes) => {
                    let doc: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                    let (msgs, cut) = gemini_messages(&doc);
                    (msgs, cut, false)
                }
                Err(_) => (Vec::new(), false, true),
            },
            None => (Vec::new(), false, true),
        },
        Source::OpenCodeDb => match opencode_messages(&roots.opencode_db, &found.session.id) {
            Ok((msgs, cut)) => (msgs, cut, false),
            Err(err) => {
                log::warn!("opencode transcript: {err}");
                (Vec::new(), false, true)
            }
        },
    };
    let mut session = found.session;
    if session.message_count.is_none() {
        session.message_count = Some(messages.len() as u32);
    }
    ChatTranscript {
        session,
        messages,
        truncated,
        missing_file,
    }
}

fn jsonl_messages(path: &Path, kind: Source) -> (Vec<ChatMessage>, bool) {
    let Ok(file) = File::open(path) else {
        return (Vec::new(), false);
    };
    let mut reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut buf = String::new();
    let mut hit_cap = false;
    loop {
        if messages.len() >= MAX_MESSAGES {
            hit_cap = true;
            break;
        }
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(v) = serde_json::from_str::<Value>(buf.trim()) else {
            continue;
        };
        if let Some(msg) = match kind {
            Source::ClaudeJsonl => claude_line(&v),
            Source::CodexJsonl => codex_line(&v),
            _ => None,
        } {
            messages.push(msg);
        }
    }
    (messages, hit_cap)
}

fn claude_line(v: &Value) -> Option<ChatMessage> {
    let kind = v.get("type").and_then(Value::as_str)?;
    if kind != "user" && kind != "assistant" {
        return None;
    }
    let message = v.get("message")?;
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or(kind);
    if role != "user" && role != "assistant" {
        return None;
    }
    let text = clip(&text_of(message.get("content").unwrap_or(&Value::Null)));
    if text.is_empty() {
        return None;
    }
    Some(ChatMessage {
        role: role.to_string(),
        text,
        at: v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn codex_line(v: &Value) -> Option<ChatMessage> {
    if v.get("type").and_then(Value::as_str) != Some("response_item") {
        return None;
    }
    let payload = v.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    let role = payload.get("role").and_then(Value::as_str)?;
    if role != "user" && role != "assistant" {
        return None;
    }
    let text = clip(&text_of(payload.get("content").unwrap_or(&Value::Null)));
    if text.is_empty() {
        return None;
    }
    Some(ChatMessage {
        role: role.to_string(),
        text,
        at: v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn gemini_messages(doc: &Value) -> (Vec<ChatMessage>, bool) {
    let Some(items) = doc.get("messages").and_then(Value::as_array) else {
        return (Vec::new(), false);
    };
    let truncated = items.len() > MAX_MESSAGES;
    let messages = items
        .iter()
        .take(MAX_MESSAGES)
        .filter_map(|m| {
            let role = m.get("role").or_else(|| m.get("type")).and_then(Value::as_str)?;
            let role = match role {
                "user" => "user",
                "model" | "gemini" | "assistant" => "assistant",
                _ => return None,
            };
            let text = clip(&message_text(m));
            if text.is_empty() {
                return None;
            }
            Some(ChatMessage {
                role: role.into(),
                text,
                at: None,
            })
        })
        .collect();
    (messages, truncated)
}

fn opencode_messages(db_path: &Path, session_key: &str) -> Result<(Vec<ChatMessage>, bool), String> {
    let Some(sid) = session_key.strip_prefix("opencode:") else {
        return Err("bad opencode id".into());
    };
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT type, data, time_created FROM session_message
             WHERE session_id = ?1 AND type IN ('user', 'assistant')
             ORDER BY seq",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([sid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut messages = Vec::new();
    let mut truncated = false;
    for row in rows {
        let (kind, data, at) = row.map_err(|e| e.to_string())?;
        if messages.len() >= MAX_MESSAGES {
            truncated = true;
            break;
        }
        let Ok(doc) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let text = if kind == "user" {
            doc.get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    doc.pointer("/metadata/displayText")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                })
        } else {
            text_of(doc.get("content").unwrap_or(&Value::Null))
        };
        let text = clip(&text);
        if text.is_empty() {
            continue;
        }
        messages.push(ChatMessage {
            role: if kind == "user" { "user" } else { "assistant" }.into(),
            text,
            at: at.and_then(epoch_to_rfc3339),
        });
    }
    Ok((messages, truncated))
}

fn message_text(m: &Value) -> String {
    if let Some(s) = m.get("content").and_then(Value::as_str) {
        return s.to_string();
    }
    if m.get("content").is_some() {
        let text = text_of(m.get("content").unwrap_or(&Value::Null));
        if !text.is_empty() {
            return text;
        }
    }
    text_of(m.get("parts").unwrap_or(&Value::Null))
}

fn text_parts(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                Vec::new()
            } else {
                vec![s.to_string()]
            }
        }
        Value::Array(items) => items
            .iter()
            .filter_map(|part| {
                if let Some(s) = part.as_str() {
                    let s = s.trim();
                    return if s.is_empty() { None } else { Some(s.to_string()) };
                }
                let obj = part.as_object()?;
                let kind = obj.get("type").and_then(Value::as_str).unwrap_or("text");
                if !matches!(kind, "text" | "input_text" | "output_text") {
                    return None;
                }
                obj.get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn text_of(v: &Value) -> String {
    text_parts(v).join("\n")
}

fn codex_transcript(dir: &Path, found: &Found) -> (Vec<ChatMessage>, bool, bool) {
    let sid = found.session.id.strip_prefix("codex:").unwrap_or("");
    match codex_db_messages(&dir.join("thread_history_1.sqlite"), sid) {
        Ok(Some((msgs, cut))) => return (msgs, cut, false),
        Ok(None) => {}
        Err(err) => log::warn!("codex history db: {err}"),
    }
    match &found.path {
        Some(path) if path.is_file() => {
            let (msgs, cut) = jsonl_messages(path, Source::CodexJsonl);
            (msgs, cut, false)
        }
        _ => (Vec::new(), false, true),
    }
}

fn codex_db_summaries(db_path: &Path) -> Result<Vec<Found>, String> {
    if !db_path.is_file() {
        return Ok(Vec::new());
    }
    let conn = open_ro(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT thread_id, MAX(created_at_ms),
                    SUM(item_type IN ('userMessage', 'agentMessage'))
             FROM thread_items
             GROUP BY thread_id",
        )
        .map_err(|e| e.to_string())?;
    let stats: Vec<(String, Option<i64>, Option<i64>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let titles = codex_db_titles(&conn)?;
    Ok(stats
        .into_iter()
        .map(|(id, updated, count)| Found {
            session: ChatSession {
                id: format!("codex:{id}"),
                client: ClientId::Codex,
                title: titles
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "未命名会话".into()),
                model: None,
                cwd: None,
                updated_at: updated.and_then(epoch_to_rfc3339),
                message_count: count.map(|n| n as u32),
            },
            path: None,
            kind: Source::CodexJsonl,
        })
        .collect())
}

fn codex_db_titles(conn: &Connection) -> Result<BTreeMap<String, String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT i.thread_id, i.item_json
             FROM thread_items i
             JOIN (
               SELECT thread_id, MIN(rollout_ordinal) AS ord
               FROM thread_items
               WHERE item_type = 'userMessage'
               GROUP BY thread_id
             ) f ON f.thread_id = i.thread_id AND f.ord = i.rollout_ordinal
             WHERE i.item_type = 'userMessage'",
        )
        .map_err(|e| e.to_string())?;
    let mut out = BTreeMap::new();
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, raw) = row.map_err(|e| e.to_string())?;
        let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let text = text_of(doc.get("content").unwrap_or(&Value::Null));
        if !text.trim().is_empty() {
            out.insert(id, title_from(&text));
        }
    }
    Ok(out)
}

fn codex_db_messages(db_path: &Path, thread_id: &str) -> Result<Option<(Vec<ChatMessage>, bool)>, String> {
    if !db_path.is_file() || thread_id.is_empty() {
        return Ok(None);
    }
    let conn = open_ro(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT item_type, item_json, created_at_ms
             FROM thread_items
             WHERE thread_id = ?1 AND item_type IN ('userMessage', 'agentMessage')
             ORDER BY rollout_ordinal",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([thread_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut messages = Vec::new();
    let mut seen = false;
    let mut truncated = false;
    for row in rows {
        seen = true;
        if messages.len() >= MAX_MESSAGES {
            truncated = true;
            break;
        }
        let (kind, raw, at) = row.map_err(|e| e.to_string())?;
        let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let text = if kind == "agentMessage" {
            doc.get("text").and_then(Value::as_str).unwrap_or("").to_string()
        } else {
            text_of(doc.get("content").unwrap_or(&Value::Null))
        };
        let text = clip(&text);
        if text.is_empty() {
            continue;
        }
        messages.push(ChatMessage {
            role: if kind == "userMessage" {
                "user"
            } else {
                "assistant"
            }
            .into(),
            text,
            at: at.and_then(epoch_to_rfc3339),
        });
    }
    if !seen {
        return Ok(None);
    }
    Ok(Some((messages, truncated)))
}

fn codex_index_names(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(text) = fs::read_to_string(path) else {
        return out;
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(id) = v.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(name) = v.get("thread_name").and_then(Value::as_str) else {
            continue;
        };
        if !name.trim().is_empty() {
            out.insert(id.to_string(), name.to_string());
        }
    }
    out
}

fn open_ro(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())
}

fn model_id_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return v
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .or_else(|| Some(raw.to_string()));
    }
    Some(raw.to_string())
}

fn title_from(text: &str) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = one.chars().count();
    let mut t: String = one.chars().take(72).collect();
    if count > 72 {
        t.push('…');
    }
    if t.is_empty() {
        "未命名会话".into()
    } else {
        t
    }
}

fn clip(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= MAX_TEXT {
        return trimmed.to_string();
    }
    let mut t: String = trimmed.chars().take(MAX_TEXT).collect();
    t.push('…');
    t
}

fn epoch_to_rfc3339(raw: i64) -> Option<String> {
    let secs = if raw.abs() > 10_000_000_000 { raw / 1000 } else { raw };
    DateTime::<Utc>::from_timestamp(secs, 0).map(|t| t.to_rfc3339())
}

fn rfc3339_mtime(path: &Path) -> Option<String> {
    let modified = path.metadata().ok()?.modified().ok()?;
    let dt: DateTime<Utc> = modified.into();
    Some(dt.to_rfc3339())
}

fn file_within(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    path.starts_with(root)
}

fn read_head(path: &Path, max: usize) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; max];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

fn collect_files(root: &Path, max_depth: usize, name_suffix: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if out.len() >= 2_000 {
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                if depth < max_depth {
                    stack.push((path, depth + 1));
                }
            } else if meta.is_file() {
                let ok = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(name_suffix));
                if ok {
                    out.push(path);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(tmp: &Path) -> HistoryRoots {
        HistoryRoots {
            claude_dir: tmp.join(".claude"),
            codex_dir: tmp.join(".codex"),
            gemini_dir: tmp.join(".gemini"),
            opencode_db: tmp.join("opencode").join("opencode.db"),
        }
    }

    #[test]
    fn reads_claude_codex_and_opencode() {
        let tmp = tempfile::tempdir().unwrap();
        let r = roots(tmp.path());

        let project = r.claude_dir.join("projects").join("-work");
        fs::create_dir_all(&project).unwrap();
        let jsonl = project.join("sess.jsonl");
        fs::write(
            &jsonl,
            "{\"type\":\"user\",\"timestamp\":\"2026-10-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"看看库存\"}}\n\
             {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"库存还够\"}]}}\n",
        )
        .unwrap();
        fs::write(
            project.join("sessions-index.json"),
            format!(
                r#"{{"entries":[{{"sessionId":"s1","fullPath":"{}","firstPrompt":"看看库存","summary":"","messageCount":2,"modified":"2026-10-01T00:00:00Z","projectPath":"/work","isSidechain":false}}]}}"#,
                jsonl.display()
            ),
        )
        .unwrap();

        let day = r.codex_dir.join("sessions/2026/10/05");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join("rollout.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"session_id\":\"cx1\",\"cwd\":\"/repo\",\"timestamp\":\"2026-10-05T01:00:00Z\"}}\n\
             {\"type\":\"turn_context\",\"payload\":{\"model\":\"code-best\",\"cwd\":\"/repo\"}}\n\
             {\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"修一下测试\"}]}}\n\
             {\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"好了\"}]}}\n",
        )
        .unwrap();
        fs::write(
            day.join("rollout-parts.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"session_id\":\"cx2\",\"cwd\":\"/repo\",\"timestamp\":\"2026-10-05T02:00:00Z\"}}\n\
             {\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"前文\"},{\"type\":\"input_text\",\"text\":\"中段\"},{\"type\":\"input_text\",\"text\":\"后段\"},{\"type\":\"input_text\",\"text\":\"真正的问题\"}]}}\n",
        )
        .unwrap();
        fs::write(
            r.codex_dir.join("session_index.jsonl"),
            "{\"id\":\"cx1\",\"thread_name\":\"索引标题\",\"updated_at\":1}\n",
        )
        .unwrap();
        let db = Connection::open(r.codex_dir.join("thread_history_1.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TABLE thread_items (
                thread_id TEXT, rollout_ordinal INTEGER, created_at_ms INTEGER,
                item_json TEXT, item_type TEXT
             );
             INSERT INTO thread_items VALUES (
                'cx1', 1, 1788145348398,
                '{\"content\":[{\"type\":\"text\",\"text\":\"数据库里的完整提问\"}]}',
                'userMessage'
             );
             INSERT INTO thread_items VALUES (
                'cx1', 2, 1788145349000,
                '{\"text\":\"数据库里的完整回答\"}',
                'agentMessage'
             );
             INSERT INTO thread_items VALUES (
                'cx-only', 1, 1788145400000,
                '{\"content\":[{\"type\":\"text\",\"text\":\"只有数据库\"}]}',
                'userMessage'
             );",
        )
        .unwrap();

        fs::create_dir_all(r.opencode_db.parent().unwrap()).unwrap();
        let conn = Connection::open(&r.opencode_db).unwrap();
        conn.execute_batch(
            "CREATE TABLE session_v2 (
                id TEXT, title TEXT, directory TEXT, model TEXT,
                time_updated INTEGER, time_created INTEGER
             );
             CREATE TABLE session_message (
                id TEXT, session_id TEXT, type TEXT, seq INTEGER,
                time_created INTEGER, data TEXT
             );
             INSERT INTO session_v2 VALUES (
                'ses_1', '改路由', '/app', '{\"id\":\"code-fast\"}', 1790000000000, 1790000000000
             );
             INSERT INTO session_message VALUES (
                'm1', 'ses_1', 'user', 1, 1790000000000, '{\"text\":\"加一个按钮\"}'
             );
             INSERT INTO session_message VALUES (
                'm2', 'ses_1', 'assistant', 2, 1790000001000,
                '{\"content\":[{\"type\":\"text\",\"text\":\"加上了\"}]}'
             );",
        )
        .unwrap();

        let chats = r.gemini_dir.join("tmp/hash/chats");
        fs::create_dir_all(&chats).unwrap();
        fs::write(
            chats.join("session-g1.json"),
            r#"{"sessionId":"g1","lastUpdated":"2026-09-01T00:00:00Z","messages":[
                {"role":"user","parts":[{"text":"解释这段"}]},
                {"role":"model","parts":[{"text":"这是入口"}]}
            ]}"#,
        )
        .unwrap();

        let listed = list(&r);
        let ids: Vec<_> = listed.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"claude:s1"), "{ids:?}");
        assert!(ids.contains(&"codex:cx1"), "{ids:?}");
        assert!(ids.contains(&"opencode:ses_1"), "{ids:?}");
        assert!(ids.contains(&"gemini:g1"), "{ids:?}");
        assert!(ids.contains(&"codex:cx-only"), "{ids:?}");
        let cx1 = listed.iter().find(|s| s.id == "codex:cx1").unwrap();
        assert_eq!(cx1.model.as_deref(), Some("code-best"));
        assert_eq!(cx1.title, "索引标题");
        assert_eq!(cx1.message_count, Some(2));

        let full = read(&r, "codex:cx1").unwrap();
        assert_eq!(full.messages.len(), 2);
        assert_eq!(full.messages[0].text, "数据库里的完整提问");
        assert_eq!(full.messages[1].text, "数据库里的完整回答");

        let parts = read(&r, "codex:cx2").unwrap();
        assert!(parts.messages[0].text.contains("前文"));
        assert!(parts.messages[0].text.contains("真正的问题"));

        let claude = read(&r, "claude:s1").unwrap();
        assert_eq!(claude.messages.len(), 2);
        assert_eq!(claude.messages[1].text, "库存还够");

        let oc = read(&r, "opencode:ses_1").unwrap();
        assert_eq!(oc.session.model.as_deref(), Some("code-fast"));
        assert_eq!(oc.messages[0].text, "加一个按钮");
        assert!(read(&r, "codex:missing").is_none());
    }
}
