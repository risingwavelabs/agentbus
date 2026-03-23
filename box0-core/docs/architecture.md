# Architecture

## System overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Your Machine                         │
│                                                             │
│   ┌─────────────────┐         ┌──────────────────────────┐ │
│   │   Your Agent    │         │       Box0 Server        │ │
│   │  (Claude Code / │──b0────▶│                          │ │
│   │   Codex / You)  │ delegate│  ┌────────┐  ┌────────┐  │ │
│   └─────────────────┘         │  │ Inbox  │  │  DB    │  │ │
│                                │  └────────┘  └────────┘  │ │
│   ┌─────────────────┐         │       ▲                   │ │
│   │   Web Dashboard │◀────────│       │                   │ │
│   │  (browser :8080)│  serves │       │ poll              │ │
│   └─────────────────┘         └───────┼───────────────────┘ │
│                                        │                     │
│              ┌─────────────────────────┼──────────────────┐ │
│              │    Node Daemon          │                   │ │
│              │                         ▼                   │ │
│              │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │ │
│              │  │ worker-1 │  │ worker-2 │  │ worker-3 │  │ │
│              │  │(ux-expert│  │(architect│  │(pragmatis│  │ │
│              │  │  Claude) │  │  Codex)  │  │   auto)  │  │ │
│              │  └──────────┘  └──────────┘  └──────────┘  │ │
│              └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Task flow

```
 Your Agent          b0 Server            Node Daemon          Claude CLI
     │                   │                     │                    │
     │  delegate(task)   │                     │                    │
     │──────────────────▶│                     │                    │
     │                   │  stores in inbox    │                    │
     │  delegate(task2)  │                     │                    │
     │──────────────────▶│                     │                    │
     │                   │                     │                    │
     │                   │◀──── poll inbox ────│                    │
     │                   │───── task1 ────────▶│                    │
     │                   │                     │  spawn subprocess  │
     │                   │                     │───────────────────▶│
     │                   │                     │  pipe task via     │
     │                   │                     │     stdin          │
     │                   │                     │                    │ (thinking)
     │                   │                     │◀── result ─────────│
     │                   │◀─── write result ───│                    │
     │                   │                     │                    │
     │  b0 wait          │                     │                    │
     │──────────────────▶│                     │                    │
     │◀─── results ──────│                     │                    │
     │                   │                     │                    │
```

## Multi-machine topology

```
                    ┌──────────────────────────┐
                    │      Box0 Server         │
                    │       Machine A          │
                    │  ┌──────────────────────┐│
                    │  │  inbox / routing     ││
                    │  └──────────────────────┘│
                    └────────────┬─────────────┘
                                 │  HTTP
              ┌──────────────────┼──────────────────┐
              │                  │                  │
    ┌─────────▼────────┐ ┌───────▼──────────┐ ┌────▼─────────────┐
    │   Machine A      │ │   Machine B      │ │   Machine C      │
    │   (local node)   │ │  (gpu-box node)  │ │  (cloud node)    │
    │                  │ │                  │ │                  │
    │ ┌──────────────┐ │ │ ┌──────────────┐ │ │ ┌──────────────┐ │
    │ │  ux-expert   │ │ │ │  ml-agent    │ │ │ │  reviewer    │ │
    │ │  architect   │ │ │ │  (GPU tasks) │ │ │ │  (cloud cred)│ │
    │ └──────────────┘ │ │ └──────────────┘ │ │ └──────────────┘ │
    │  own credentials │ │  own credentials │ │  own credentials │
    └──────────────────┘ └──────────────────┘ └──────────────────┘
```

## Components

**Server** (`src/server.rs`). Axum HTTP server. Handles API requests, serves the web dashboard, and manages auth middleware. Routes: workers, tasks, nodes, users, groups, skills.

**Database** (`src/db.rs`). SQLite with WAL mode. Tables: `users`, `groups`, `group_members`, `agents`, `inbox_messages`, `nodes`, `workers`. Group names used as tenants for isolation.

**Daemon** (`src/daemon.rs`). Polls worker inboxes every 2 seconds. Spawns Claude Code or Codex as subprocesses in each worker's isolated directory. Two modes:
- **Local daemon**: runs inside the server process, direct DB access.
- **Remote daemon**: runs on remote nodes, communicates with server via HTTP.

Max concurrency: 4 concurrent tasks. Timeout: 300 seconds per task.

**CLI** (`src/main.rs`). Entry point for all subcommands. HTTP client communicates with the server.

**Config** (`src/config.rs`). Server config (host, port, DB path) and CLI config (server URL, API key, default group). Skill installation for Claude Code and Codex.

## Data model

- **Users** have unique API keys. Keys identify users, not groups.
- **Groups** provide tenant isolation. Each user gets a personal group on creation.
- **Workers** belong to a group and are registered by a specific user. Only the creator can modify them.
- **Inbox messages** are the task queue. Each message targets a worker and carries the task content.
- **Nodes** are machines running worker processes. Owned by the user who joined them.

## Worker execution

1. Task arrives in worker's inbox via `b0 delegate`.
2. Daemon picks up the task and spawns the configured runtime:
   - Claude Code: `claude --print --output-format json --system-prompt "<instructions>"`, task piped via stdin.
   - Codex: `codex exec --json --full-auto --skip-git-repo-check "<instructions>\n\n<task>"`.
3. Runtime output is parsed and stored as the response.
4. For multi-turn conversations, the Claude session ID is stored and used with `--resume` on follow-up messages. Codex does not support session resume.

## Auth model

- Users authenticate via API key in the `Authorization` header.
- Each user can be in multiple groups. `--group` selects the operating context.
- Workers track `registered_by`. Only the creator can remove, update, or stop their workers.
- Nodes are owned by users. Only the owner can deploy workers to their node.
- Admin user is created on first server start.
