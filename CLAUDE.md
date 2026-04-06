# CLAUDE.md

## Project

Box0 is a multi-agent platform. It lets you run multiple AI agents in parallel across machines. Rust codebase, single binary (`b0`). npm package: `@box0/cli`.

## Conventions

- Never use em dashes. Use periods, commas, colons, or "- " instead.
- All content in English.
- README code blocks must be copy-paste safe. No inline comments on the same line as commands. No blocking commands (like `b0 server`) in the same block as other commands. Use blockquotes for conversation examples, not code blocks.
- Always test changes before committing. Run `cargo test` at minimum (unit + integration tests). For user-facing features, run `tests/e2e.sh` which requires Claude Code or Codex.
- After code changes, always update README.md, CLAUDE.md, docs/, and the skill content in config.rs if affected. This is critical.
- SKILL.md (repo root) is the canonical skill definition. Any change to CLI commands, flags, or agent behavior must be reflected in it.
- Commit messages: imperative mood, concise first line, details in body.
- No documentation files unless explicitly requested.

## Architecture

- `src/lib.rs` - Library crate, re-exports all modules
- `src/main.rs` - CLI entry point, all subcommand dispatch
- `src/server.rs` - Axum HTTP server, route handlers, auth middleware, `build_router()` for tests
- `src/db.rs` - SQLite schema, models, all queries
- `src/daemon.rs` - Daemon (local + remote), processes agent inboxes, spawns runtime
- `src/client.rs` - HTTP client for CLI-to-server communication
- `src/config.rs` - Server config, CLI config, skill installation, pending state
- `src/scheduler.rs` - Cron job scheduler, runs recurring tasks on interval

## Resource model

- **Workspaces** are logical groups for organizing agents, tasks, and team access.
- **Agents** belong to a workspace. Workspace controls visibility.

## Auth model

- Users have unique keys. Keys identify users, not workspaces.
- Each user gets a personal workspace on creation.
- Users can be in multiple workspaces. `--workspace` flag selects which workspace to operate in. Defaults to `default_workspace` in config.
- `b0 login` auto-sets `default_workspace` from user's first workspace. No need for manual config.
- Agents track `registered_by`. Only the creator can remove/update/stop their agents.
- Admin user is created on first server start. Server auto-writes CLI config (no login needed on server machine).

## Agent execution

- Each agent has its own isolated directory under `~/.b0/agents/<name>/`.
- Agents support multiple runtimes: `auto` (default), `claude`, or `codex`.
  - `auto` prefers Claude Code if installed, falls back to Codex.
  - Set per-agent via `--runtime claude` or `--runtime codex`.
- Daemon spawns the runtime CLI in the agent's directory.
  - Claude: `claude --print --output-format json --system-prompt "<instructions>"`, task piped via stdin.
  - Codex: `codex exec --json --full-auto --skip-git-repo-check [-C <dir>] "<instructions>\n\n<task>"`, task as argument.
  - Codex output is JSONL. Parse `item.completed` events, extract `item.text`.
  - Codex requires `--skip-git-repo-check` because agent directories are not git repos.
- Session IDs are tracked per thread for multi-turn conversations (`--resume`, Claude only). Codex does not support session resume.
- Multi-turn: `b0 delegate --thread <id>` sends "answer" message, daemon resumes Claude session.
- Windows compatibility: runtime detection uses `where` instead of `which`.
- On completion, webhooks are fired and Slack notifications sent if configured on the agent.
- Background agents support three trigger types: Manual (`b0 delegate`), Cron (`b0 cron add`), Webhook (POST `/trigger/<workspace>/<agent>`).
- Every agent has a deterministic trigger URL: `POST /trigger/<workspace>/<agent-name>` (no auth required). Optional HMAC secret stored as `webhook_secret` on the agent.
- Agent kinds: `background` (persistent, triggered on demand) and `cron` (scheduled, auto-created by `b0 cron add`).

## CLI design

- `--workspace` is optional when `default_workspace` is set in config.
- `b0 server` on first start auto-configures `~/.b0/config.toml` (server_url, api_key, default_workspace).
- `b0 login` on remote machines auto-sets default_workspace from user's first workspace.
- `b0 delegate` without `--thread` creates new conversation. With `--thread` continues existing one.
- `b0 agent add --webhook-secret <secret>` sets an HMAC secret for the agent's trigger URL. Trigger URL is printed on registration and visible in `b0 agent info`.
- Skills are installed via `npx skills add risingwavelabs/skills --skill b0`, not via the b0 CLI.
- Claude Code skill lives at `~/.claude/skills/b0/SKILL.md` (directory format, not plain file).
- Codex skill lives in `~/.codex/AGENTS.md`.

## Distribution

- npm package: `@box0/cli`
- Install: `npm install -g @box0/cli@latest`
- CI builds 5 platforms on tag push: darwin-arm64, darwin-x64, linux-x64, linux-arm64, windows-x64
- npm version auto-synced from git tag in CI
- `install.js` downloads binary from GitHub releases matching package.json version
- Release flow: `git tag v0.x.0 && git push origin v0.x.0`

## Task system

- Users interact with Box0 via Tasks. Agents are invisible infrastructure.
- Each task has: title, status (running/needs_input/done/failed), conversation thread, optional sub-tasks, result.
- Web UI: left panel = chat, right panel = task board grouped by status.
- Creating a task via Web UI auto-creates a background agent to handle it.
- Two paths: CLI (`b0 delegate`) and Web UI (Task API). Both converge on the same inbox/daemon layer.
- Task status auto-updates when inbox messages of type "done", "failed", or "question" arrive on the task's thread.
- Agent timeout is configurable per-agent (default 300s).

## DB schema

Tables: users, workspaces, workspace_members, agents, inbox_messages, machines, tasks, cron_jobs, workflows, workflow_nodes, workflow_edges, workflow_runs, workflow_step_runs. Workspace name is used as tenant for isolation. Agents have a `webhook_secret` column for HMAC verification of trigger requests.

## Testing

- Unit tests in `src/db.rs` (14 tests covering users, workspaces, agents, inbox, machines, ownership, tasks, workflows).
- API integration tests in `tests/api.rs` (13 tests). Start a real Axum server per test with temp DB, test via HTTP client. No Claude/Codex needed. Run with `cargo test`.
- E2e script in `tests/e2e.sh`. Requires Claude Code or Codex installed. Starts real server, runs CLI commands, verifies results. Run manually before releases.
- CI runs `cargo test` on every push/PR via `.github/workflows/ci.yml`.
- `b0 reset` deletes DB, config, and skills for clean slate.
