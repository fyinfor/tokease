//! Local agent skills (`SKILL.md`) for the tools installed on this machine.
//!
//! Read-only. A skill is the directory that contains `SKILL.md`. System
//! skills, installed user skills, and plugin skills are all listed. The
//! scan stays inside the known roots and does not follow directory symlinks.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_DEPTH: u8 = 10;
const HEAD_BYTES: usize = 8 * 1024;
const MAX_DESC: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    /// `agents`, `codex`, `claude`, or `cursor`.
    pub source: String,
    /// `user`, `system`, or `plugin`.
    pub kind: String,
    /// Directory that holds `SKILL.md`.
    pub path: String,
}

#[derive(Clone)]
struct Root {
    source: &'static str,
    dir: PathBuf,
    plugin: bool,
}

/// Skills under the current user's home directory.
pub fn list() -> Vec<LocalSkill> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    list_in(&[
        Root { source: "agents", dir: home.join(".agents/skills"), plugin: false },
        Root { source: "codex", dir: home.join(".codex/skills"), plugin: false },
        Root { source: "codex", dir: home.join(".codex/plugins"), plugin: true },
        Root { source: "claude", dir: home.join(".claude/skills"), plugin: false },
        Root { source: "claude", dir: home.join(".claude/plugins"), plugin: true },
        Root { source: "cursor", dir: home.join(".cursor/skills"), plugin: false },
        Root { source: "cursor", dir: home.join(".cursor/skills-cursor"), plugin: false },
    ])
}

fn list_in(roots: &[Root]) -> Vec<LocalSkill> {
    let mut out = Vec::new();
    for root in roots {
        if !root.dir.is_dir() {
            continue;
        }
        walk(root, &root.dir, 0, &mut out);
    }
    out.sort_by(|a, b| (&a.source, &a.name, &a.path).cmp(&(&b.source, &b.name, &b.path)));
    out
}

fn walk(root: &Root, dir: &Path, depth: u8, out: &mut Vec<LocalSkill>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "node_modules" {
            continue;
        }
        if meta.is_dir() {
            walk(root, &path, depth + 1, out);
        } else if name.eq_ignore_ascii_case("SKILL.md") {
            if let Some(skill) = read_skill(root, &path) {
                out.push(skill);
            }
        }
    }
}

fn read_skill(root: &Root, file: &Path) -> Option<LocalSkill> {
    let dir = file.parent()?;
    let rel = dir.strip_prefix(&root.dir).unwrap_or(dir);
    let head = read_head(file);
    let fm = frontmatter(&head);
    let folder = dir.file_name().and_then(|s| s.to_str()).unwrap_or("skill");
    let name = fm
        .and_then(|text| field(text, "name"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| folder.to_string());
    let description = fm.and_then(|text| field(text, "description")).unwrap_or_default();
    let kind = if rel.components().any(|c| c.as_os_str() == ".system") || root.dir.ends_with("skills-cursor") {
        "system"
    } else if root.plugin {
        "plugin"
    } else {
        "user"
    };
    Some(LocalSkill {
        id: file.to_string_lossy().into_owned(),
        name,
        description: clip(&description),
        source: root.source.to_string(),
        kind: kind.to_string(),
        path: dir.to_string_lossy().into_owned(),
    })
}

fn read_head(path: &Path) -> String {
    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let mut buf = Vec::new();
    let _ = file.take(HEAD_BYTES as u64).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn frontmatter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn field(fm: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let lines: Vec<&str> = fm.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim().strip_prefix(&prefix) else {
            continue;
        };
        let rest = rest.trim();
        if rest == ">" || rest == "|" || rest == ">-" || rest == "|-" {
            let mut parts = Vec::new();
            for next in lines.iter().skip(i + 1) {
                if next.starts_with(' ') || next.starts_with('\t') {
                    let piece = next.trim();
                    if !piece.is_empty() {
                        parts.push(piece);
                    }
                } else if next.trim().is_empty() {
                    continue;
                } else {
                    break;
                }
            }
            return Some(parts.join(" "));
        }
        if rest.is_empty() {
            return None;
        }
        return Some(rest.trim_matches(|c| c == '"' || c == '\'').to_string());
    }
    None
}

fn clip(text: &str) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = one.chars().count();
    let mut out: String = one.chars().take(MAX_DESC).collect();
    if count > MAX_DESC {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    #[test]
    fn reads_user_system_and_plugin_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let agents = tmp.path().join("agents");
        write_skill(
            &agents.join("brandkit"),
            "---\nname: brandkit\ndescription: >\n  Premium boards\n  and logos\n---\n# Body\n",
        );
        let codex = tmp.path().join("codex");
        write_skill(
            &codex.join(".system/review-agent"),
            "---\nname: review-agent\ndescription: \"Review a diff\"\n---\n",
        );
        let plugins = tmp.path().join("plugins");
        write_skill(
            &plugins.join("cache/pub/demo/1/skills/sites-building"),
            "---\nname: sites-building\ndescription: Build a site\n---\n",
        );
        fs::create_dir_all(plugins.join("cache/pub/demo/1/node_modules/hidden")).unwrap();
        write_skill(&plugins.join("cache/pub/demo/1/node_modules/hidden"), "---\nname: hidden\ndescription: no\n---\n");

        let found = list_in(&[
            Root { source: "agents", dir: agents, plugin: false },
            Root { source: "codex", dir: codex, plugin: false },
            Root { source: "codex", dir: plugins, plugin: true },
        ]);
        let names: Vec<_> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["brandkit", "review-agent", "sites-building"]);
        assert_eq!(found[0].description, "Premium boards and logos");
        assert_eq!(found[0].kind, "user");
        assert_eq!(found[0].source, "agents");
        assert_eq!(found[1].kind, "system");
        assert_eq!(found[2].kind, "plugin");
        assert!(found[2].path.ends_with("sites-building"));
    }

    #[test]
    fn this_machine_has_the_installed_agent_skills() {
        let home = dirs::home_dir().unwrap();
        if !home.join(".agents/skills/brandkit/SKILL.md").is_file() {
            return;
        }
        let found = list();
        assert!(found.iter().any(|s| s.source == "agents" && s.name == "brandkit"));
        assert!(found.iter().any(|s| s.source == "codex" && s.kind == "system"));
        assert!(found.len() > 20, "found {}", found.len());
    }
}
