export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: dev release compliance

# 编译并启动开发版桌面应用
dev:
	@if [ -d .git ]; then ln -sfn ../../.githooks/pre-commit .git/hooks/pre-commit; fi
	pnpm tauri:dev

# 编译发布版，产物在 src-tauri/target/release/bundle/
release:
	pnpm tauri:build

# 合规检查：官方 CLI + Tokease Gateway Key。git pre-commit 跑同一条命令。
compliance:
	@if [ -d .git ]; then ln -sfn ../../.githooks/pre-commit .git/hooks/pre-commit; fi
	python3 scripts/compliance_check.py
