export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: dev release

# 编译并启动开发版桌面应用
dev:
	pnpm tauri:dev

# 编译发布版，产物在 src-tauri/target/release/bundle/
release:
	pnpm tauri:build
