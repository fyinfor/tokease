#!/usr/bin/env python3
"""Pre-commit gate for the Tokease compliance architecture.

Allowed shape:
  official CLI + Tokease custom base URL + the user's own Tokease gateway key.
  Upstream tokease.com is an accepted commercial API, including downstream use.

The check fails the commit when source leaves that shape.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

CONDITIONS = (
    "1. 官方 CLI 不被魔改成绕过认证/限制的版本",
    "2. CLI 使用的是 Tokease Gateway Key",
    "3. Tokease Key 只是你自己的用户认证凭据",
    "4. 不共享/转售上游 API key",
    "5. 不接 Claude Pro/Max、ChatGPT Plus/Pro 的消费者 OAuth 号池",
    "6. 不绕过 rate limit、安全限制或供应商使用政策",
)

VERDICT = "这是相对标准、可落地的合规架构。"

CODE_SUFFIXES = {".rs", ".ts", ".tsx", ".js", ".mjs", ".py"}
SKIP_DIRS = {"target", "node_modules", "dist", ".git"}
SKIP_FILES = {
    "scripts/compliance_check.py",
}

# Credential fields this program is allowed to write, and only from spec.token.
CREDENTIAL_FIELDS = (
    "ANTHROPIC_AUTH_TOKEN",
    "experimental_bearer_token",
    "inferenceGatewayApiKey",
    "api_key",
    "apiKey",
    "KEY_TOKEN",
)
WRITE_OPENERS = ("env(", "put_value(", "set(", ".insert(", "json!(")
CONSUMER_OAUTH = (
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "CLAUDE_CODE_OAUTH_SCOPES",
    "id_token",
    "refresh_token",
)

SKIP_AUTH_ENABLE = re.compile(
    r"""CLAUDE_CODE_SKIP_[A-Z0-9_]*AUTH["']?\s*(?:[:=]|,\s*)\s*["']?(?:1|true|yes|on)\b""",
    re.IGNORECASE,
)
OAUTH_ENV_WRITE = re.compile(r"""env\(\s*["']CLAUDE_CODE_OAUTH_""")
OAUTH_ASSIGN = re.compile(
    r"""["']CLAUDE_CODE_OAUTH_(?:TOKEN|REFRESH_TOKEN|SCOPES)["']\s*[:=]\s*["'][^"']+["']"""
)
POOL = re.compile(
    r"(?i)(?:oauth[\s_]*pool|token[\s_]*pool|account[\s_]*pool|subscription[\s_]*pool|号池)"
)
UPSTREAM_KEY = re.compile(
    r"\b(?:sk-ant-|sk-proj-|sk-or-v1-|sk-svcacct-)[A-Za-z0-9_\-]{20,}"
)
RATE_BYPASS = re.compile(
    r"(?i)(?:bypass|ignore|disable|skip)[\s_\-]{0,12}rate[\s_\-]?limits?"
    r"|rate[\s_\-]?limits?[\s_\-]{0,12}(?:bypass|ignore|disable|skip)"
)
BINARY_PATCH = re.compile(
    r"(?i)(?:patchelf|bspatch|xdelta|binary[\s_]+patch|patch[\s_]+the[\s_]+(?:claude|codex)[\s_]+binary)"
)
UPSTREAM_HOST = re.compile(
    r"https://(?:api\.openai\.com|api\.anthropic\.com|chatgpt\.com)\b"
)


def strip_comments(text: str, suffix: str) -> str:
    """Drop comments without treating https:// as a line comment."""
    if suffix not in {".rs", ".ts", ".tsx", ".js", ".mjs", ".py"}:
        return text
    hash_comments = suffix == ".py"
    out: list[str] = []
    i = 0
    n = len(text)
    while i < n:
        if not hash_comments and text.startswith("/*", i):
            end = text.find("*/", i + 2)
            if end < 0:
                break
            out.append("\n" * text.count("\n", i, end))
            i = end + 2
            continue
        if text.startswith(('r#"', "r##\""), i) or (
            text.startswith('r"', i) and suffix == ".rs"
        ):
            if text.startswith('r#"', i):
                end = text.find('"#', i + 3)
                step = 2
            elif text.startswith('r##"', i):
                end = text.find('"##', i + 4)
                step = 3
            else:
                end = text.find('"', i + 2)
                step = 1
            if end < 0:
                out.append(text[i:])
                break
            out.append(text[i : end + step])
            i = end + step
            continue
        ch = text[i]
        if ch in "\"'`":
            out.append(ch)
            i += 1
            while i < n:
                if text[i] == "\\":
                    out.append(text[i : i + 2])
                    i += 2
                    continue
                out.append(text[i])
                if text[i] == ch:
                    i += 1
                    break
                i += 1
            continue
        if not hash_comments and text.startswith("//", i):
            while i < n and text[i] != "\n":
                i += 1
            continue
        if hash_comments and ch == "#":
            while i < n and text[i] != "\n":
                i += 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def statements_from(lines: list[str]) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if any(op in line for op in WRITE_OPENERS):
            start = i
            chunk = [line]
            depth = line.count("(") - line.count(")")
            j = i + 1
            while j < len(lines) and j < i + 8 and (depth > 0 or not chunk[-1].rstrip().endswith(";")):
                chunk.append(lines[j])
                depth += lines[j].count("(") - lines[j].count(")")
                if depth <= 0 and lines[j].rstrip().endswith(";"):
                    j += 1
                    break
                j += 1
            found.append((start + 1, "\n".join(chunk)))
            i = max(j, i + 1)
            continue
        i += 1
    return found


def check_text(rel: str, text: str) -> list[str]:
    suffix = Path(rel).suffix
    code = strip_comments(text, suffix)
    lines = code.splitlines()
    problems: list[str] = []

    def add(line_no: int, condition: str, detail: str) -> None:
        problems.append(f"{rel}:{line_no}: 违反条件 {condition}: {detail}")

    for n, line in enumerate(lines, 1):
        if SKIP_AUTH_ENABLE.search(line):
            add(n, "1/6", "把 CLAUDE_CODE_SKIP_*_AUTH 写成开启，等于绕过官方 CLI 的认证")
        if OAUTH_ENV_WRITE.search(line) or OAUTH_ASSIGN.search(line):
            add(n, "5", "写入了 Claude 消费者 OAuth 凭据")
        if POOL.search(line):
            add(n, "5", "出现消费者 OAuth / 订阅号池")
        if UPSTREAM_KEY.search(line):
            add(n, "4", "源码里写了上游 API key")
        if RATE_BYPASS.search(line):
            add(n, "6", "出现绕过 rate limit 的逻辑")
        if BINARY_PATCH.search(line):
            add(n, "1", "出现修改官方 CLI 二进制的逻辑")

    for line_no, stmt in statements_from(lines):
        if any(field in stmt for field in CREDENTIAL_FIELDS) and "spec.token" not in stmt:
            # Reads and clear-lists are not write statements. A write names the
            # field and builds a value. Require the gateway key at that point.
            if any(op in stmt for op in ("env(", "put_value(", "json!(", ".insert(", "set(")):
                if re.search(r"\b(?:get|env_str|str_at|contains)\s*\(", stmt) and "spec.token" not in stmt:
                    # A read of the field inside a larger call. Only flag when
                    # the statement also assigns a credential value.
                    if not re.search(r"(?:put_value|env|json!|\.insert|set)\s*\(", stmt):
                        continue
                if "spec.token" not in stmt and re.search(
                    r"(?:ANTHROPIC_AUTH_TOKEN|experimental_bearer_token|inferenceGatewayApiKey|api_key|apiKey|KEY_TOKEN)",
                    stmt,
                ):
                    # Ignore statements that only mention the field as a path being read.
                    if re.search(r"(?:put_value|env|\.insert|set)\s*\(", stmt) or "json!(spec" in stmt:
                        add(line_no, "2/3", "写入 CLI 的凭据不是 spec.token（用户自己的 Tokease Gateway Key）")
        if "requires_openai_auth" in stmt and re.search(r"\bfrom\(\s*true\s*\)", stmt):
            add(line_no, "5", "requires_openai_auth 被固定为 true，会接上 ChatGPT 消费者登录")
        if any(field in stmt for field in CREDENTIAL_FIELDS) and any(name in stmt for name in CONSUMER_OAUTH):
            if "spec.token" in stmt and re.search(r"CLAUDE_CODE_OAUTH_|refresh_token|id_token", stmt):
                add(line_no, "5", "网关密钥和消费者 OAuth 令牌写在同一次凭据赋值里")
        if UPSTREAM_HOST.search(stmt) and any(op in stmt for op in WRITE_OPENERS):
            add(line_no, "2", "把官方 CLI 的请求地址写成了上游本站，而不是 Tokease 网关")

    return problems


def source_files() -> list[Path]:
    tracked = subprocess.run(
        ["git", "ls-files"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    files: list[Path] = []
    for rel in tracked:
        path = ROOT / rel
        if path.suffix not in CODE_SUFFIXES:
            continue
        if rel in SKIP_FILES:
            continue
        if any(part in SKIP_DIRS for part in Path(rel).parts):
            continue
        files.append(path)
    return files


def scan() -> list[str]:
    problems: list[str] = []
    for path in source_files():
        rel = path.relative_to(ROOT).as_posix()
        problems.extend(check_text(rel, path.read_text(encoding="utf-8")))
    return problems


def self_test() -> list[str]:
    bad = {
        "bad/skip.rs": 'env("CLAUDE_CODE_SKIP_BEDROCK_AUTH", "1");\n',
        "bad/oauth.rs": 'env("CLAUDE_CODE_OAUTH_TOKEN", &shared);\n',
        "bad/pool.rs": "let oauth_pool = accounts;\n",
        "bad/key.rs": 'const KEY: &str = "sk-ant-api03-ABCDEFGHIJKLMNOPQRST";\n',
        "bad/rate.rs": "fn bypass_rate_limit() {}\n",
        "bad/patch.rs": "fn patch_the_claude_binary() {}\n",
        "bad/token.rs": 'env("ANTHROPIC_AUTH_TOKEN", &upstream_key);\n',
        "bad/chatgpt.rs": 't.insert("requires_openai_auth", Item::Value(Value::from(true)));\n',
        "bad/host.rs": 'env("ANTHROPIC_BASE_URL", "https://api.anthropic.com");\n',
    }
    good = {
        "ok/claude.rs": 'env("ANTHROPIC_AUTH_TOKEN", &spec.token),\n',
        "ok/clear.rs": '"CLAUDE_CODE_OAUTH_TOKEN",\n"CLAUDE_CODE_SKIP_BEDROCK_AUTH",\n',
        "ok/codex.rs": 't.insert(\n    "experimental_bearer_token",\n    Item::Value(Value::from(spec.token.as_str())),\n);\n',
        "ok/read.rs": 'Self::env_str(&doc, "ANTHROPIC_AUTH_TOKEN").is_some()\n',
        "ok/authflag.rs": 't.insert(\n    "requires_openai_auth",\n    Item::Value(Value::from(login_on_disk)),\n);\n',
    }
    errors: list[str] = []
    for name, text in bad.items():
        if not check_text(name, text):
            errors.append(f"self-test: {name} 应该被拒绝")
    for name, text in good.items():
        hit = check_text(name, text)
        if hit:
            errors.append(f"self-test: {name} 不应被拒绝: {hit}")
    return errors


def check_license() -> list[str]:
    """MIT notice for this repo and for the CC Switch portions must stay put."""
    problems: list[str] = []
    license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    required = (
        "Copyright (c) 2026 fyinfor",
        "Copyright (c) 2025 Jason Young",
        "https://github.com/farion1231/cc-switch",
        "Permission is hereby granted, free of charge",
    )
    for needle in required:
        if license_text.count(needle) < (2 if needle.startswith("Permission") else 1):
            problems.append(f"LICENSE: 缺少「{needle}」")
    if "AGPL" in license_text or "GPL-3.0" in license_text:
        problems.append("LICENSE: 仍包含 AGPL/GPL 声明")
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    package = (ROOT / "package.json").read_text(encoding="utf-8")
    if 'license = "MIT"' not in cargo:
        problems.append('Cargo.toml: license 不是 "MIT"')
    if '"license": "MIT"' not in package:
        problems.append('package.json: license 不是 "MIT"')
    if "AGPL" in cargo or "AGPL" in package:
        problems.append("Cargo.toml 或 package.json 仍写着 AGPL")
    return problems


def main() -> int:
    problems = self_test() + scan()
    license_problems = check_license()
    if not problems and not license_problems:
        return 0
    if license_problems:
        print("许可证声明不完整：", file=sys.stderr)
        for item in license_problems:
            print(item, file=sys.stderr)
        print("", file=sys.stderr)
    if not problems:
        return 1
    print("提交被合规检查拦住。Tokease 只接受下面这组条件同时成立：", file=sys.stderr)
    for line in CONDITIONS:
        print(f"  {line}", file=sys.stderr)
    print(f"满足时：{VERDICT}", file=sys.stderr)
    print("官方 CLI + Tokease 自定义 Base URL + Tokease Gateway Key 可以保留。", file=sys.stderr)
    print("", file=sys.stderr)
    for item in problems:
        print(item, file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
