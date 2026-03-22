# CLAUDE.md

## Project

Box0 is a multi-agent platform. It lets you run multiple AI agents in parallel across machines. Rust codebase, single binary (`b0`).

## Conventions

- Never use em dashes. Use periods, commas, colons, or "- " instead.
- All content in English.
- README code blocks must be copy-paste safe. No inline comments on the same line as commands. No blocking commands (like `b0 server`) in the same block as other commands. Use blockquotes for conversation examples, not code blocks.
- Always test changes before committing. Run `cargo build` and `cargo test` at minimum. For user-facing features, run an e2e test.
- Commit messages: imperative mood, concise first line, details in body.
- No documentation files unless explicitly requested.

## Architecture

- `src/main.rs` - CLI entry point, all subcommand dispatch
- `src/server.rs` - Axum HTTP server, route handlers, auth middleware
- `src/db.rs` - SQLite schema, models, all queries
- `src/daemon.rs` - Node daemon, polls worker inboxes, spawns Claude CLI
- `src/client.rs` - HTTP client for CLI-to-server communication
- `src/config.rs` - Server config, CLI config, skill installation, pending state

## Auth model

- Users have unique keys. Keys identify users, not groups.
- Each user gets a personal group on creation.
- Users can be in multiple groups. `--group` flag selects which group to operate in. Defaults to `default_group` in config.
- Nodes are owned by users. Only the owner can deploy workers to their node.
- Workers track `registered_by`. Only the creator can remove/update/stop their workers.
- Admin user is created on first server start. Server auto-writes CLI config (no login needed on server machine).

## Worker execution

- Each worker has its own isolated directory under `workers/<name>/`.
- Daemon spawns `claude --print --output-format json --system-prompt "<instructions>"` in the worker's directory.
- Task content is piped via stdin.
- Session IDs are tracked per thread for multi-turn conversations (`--resume`).
- Multi-turn: `b0 delegate --thread <id>` sends "answer" message, daemon resumes Claude session.

## CLI design

- `--group` is optional when `default_group` is set in config.
- `b0 server` on first start auto-configures `~/.b0/config.toml`.
- `b0 worker temp` is non-blocking (same as `b0 delegate`). Temp workers auto-cleanup on `b0 wait`.
- `b0 delegate` without `--thread` creates new conversation. With `--thread` continues existing one.
- `b0 skill install claude-code` writes `~/.claude/skills/b0/SKILL.md` (directory format, not plain file).
- `b0 skill install codex` appends marked section to `~/.codex/AGENTS.md`.

## Distribution

- npm package: `@box0/cli`
- CI builds 5 platforms on tag push: darwin-arm64, darwin-x64, linux-x64, linux-arm64, windows-x64
- npm version auto-synced from git tag in CI
- `install.js` downloads binary from GitHub releases matching package.json version

## DB schema

Tables: users, groups, group_members, agents, inbox_messages, nodes, workers. Group name is used as tenant for isolation (previously called "tenant" in code, now "group_name").

## Testing

- Unit tests in `src/db.rs` (8 tests covering users, groups, workers, inbox, nodes, ownership).
- E2e tests: start server, run CLI commands, verify results. Always clean up (delete DB, config).
- `b0 reset` deletes DB, config, and skills for clean slate.
