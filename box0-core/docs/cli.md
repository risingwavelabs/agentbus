# CLI reference

## Server

```
b0 server [--host] [--port] [--db]         Start server
```

## Authentication

```
b0 login <url> --key <key>                 Connect from another machine
b0 logout                                  Disconnect
b0 reset                                   Clean slate (deletes DB, config, skills)
b0 status                                  Show connection info
b0 invite <name>                           Create user (admin only)
```

## Workers

```
b0 worker add <name> --instructions "..." [--description "..."] [--group <g>] [--node <n>] [--runtime auto|claude|codex]
b0 worker ls [--group <g>]
b0 worker info <name> [--group <g>]
b0 worker update <name> [--group <g>]
b0 worker stop <name> [--group <g>]
b0 worker start <name> [--group <g>]
b0 worker logs <name> [--group <g>]
b0 worker remove <name> [--group <g>]
b0 worker temp "<task>" [--group <g>]      One-off task (non-blocking)
```

## Task delegation

These commands are primarily used by agents, not humans.

```
b0 delegate <worker> "<task>" [--group <g>]       New task (non-blocking)
b0 delegate --thread <id> <worker> "<message>"    Continue conversation
b0 delegate <worker>                              Read task from stdin
b0 wait                                           Collect results
b0 reply [--group <g>] <thread-id> "<answer>"     Answer a worker question
```

### How delegation works

1. `b0 delegate` sends a task to a worker's inbox and returns immediately with a thread ID.
2. The node daemon picks up the task, spawns a Claude Code or Codex process, and executes it.
3. `b0 wait` blocks until all pending tasks have results, then prints them.
4. For multi-turn conversations, pass `--thread <id>` to continue an existing conversation. The worker resumes its Claude session with full history.

## Nodes

```
b0 node join <url> [--name] [--key]        Join as worker node
b0 node ls                                 List nodes
```

## Groups

```
b0 group create <name>                     Create group
b0 group ls                                List your groups
b0 group add-member <group> <user-id>      Add user to group
```

## Skills

```
b0 skill install claude-code               Install Box0 skill for Claude Code
b0 skill install codex                     Install Box0 skill for Codex
b0 skill uninstall <agent>                 Remove installed skill
b0 skill show                              Print skill content to stdout
```

### What skills do

Skills teach your agent how to use Box0. When installed:

- **Claude Code**: writes a skill file to `~/.claude/skills/b0/SKILL.md`. Claude Code reads this and learns the `b0 delegate` / `b0 wait` workflow.
- **Codex**: appends a marked section to `~/.codex/AGENTS.md`.

After installation, your agent knows how to create workers, delegate tasks, and collect results without any manual instruction.
