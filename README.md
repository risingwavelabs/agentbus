# Box0

Box0 is an agent platform for deploying and managing AI workers across machines. It provides the infrastructure to run multiple specialized AI agents, delegate tasks to them, and collect results. Any agent with shell access (Claude Code, Codex, or a custom script) can act as a lead.

## What it does

- **Worker management.** Define named agents with specialized instructions. Deploy them on one machine or distribute across many.
- **Task delegation.** Send tasks to workers from the CLI. Non-blocking. Workers execute in parallel.
- **Multi-machine.** Workers run on remote nodes. The server routes tasks. Each node uses its own local credentials and compute.
- **Multi-tenant.** Groups provide full isolation. Multiple teams share one server without seeing each other's workers or data.
- **Agent integration.** First-class skill installation for Claude Code and Codex. Your lead agent learns to delegate automatically.

## Quick example: three agents debate "Claude Code vs Codex"

```bash
# Start the server (one-time setup)
b0 server

# In another terminal: login and create workers
b0 login http://localhost:8080 --key <admin-key>
b0 group create my-team
b0 group invite my-team --description "me"
b0 login http://localhost:8080 --key <group-key>

b0 worker add ux-expert \
  --instructions "You are a UX researcher. Evaluate developer tools from the perspective of daily workflow, ergonomics, and productivity."

b0 worker add architect \
  --instructions "You are a software architect. Evaluate developer tools from the perspective of technical capabilities, extensibility, and system design."

b0 worker add pragmatist \
  --instructions "You are a pragmatic tech lead. Cut through hype. Evaluate based on what actually ships faster with fewer bugs."

# Fire off three parallel tasks
b0 delegate ux-expert "Compare Claude Code and OpenAI Codex CLI. Which one provides a better developer experience and why?"
b0 delegate architect "Compare Claude Code and OpenAI Codex CLI from a technical architecture perspective. Strengths, weaknesses, trade-offs."
b0 delegate pragmatist "Claude Code vs Codex CLI: which one actually makes engineers more productive? Be honest and specific."

# Wait for all three to come back
b0 wait
# ux-expert done (45s): Claude Code's 1M context window means...
# architect done (52s): Codex's sandbox-first approach provides...
# pragmatist done (38s): In practice, Claude Code ships faster for...
```

Three independent AI agents, three perspectives, running in parallel. You get a real debate instead of one agent's opinion.

## End-to-End Setup

### 1. Build

```bash
git clone https://github.com/risingwavelabs/box0.git
cd box0
cargo build --release
export PATH="$PWD/target/release:$PATH"
```

### 2. Start Server

```bash
b0 server
# First start prints:
#   Admin key: b0_abc123...
#   Save this key. Use it to login:
#   b0 login http://127.0.0.1:8080 --key b0_abc123...
```

### 3. Login and Create a Group

```bash
b0 login http://localhost:8080 --key b0_abc123...
b0 group create my-team
b0 group invite my-team --description "me"
#   Key: b0_def456...
b0 login http://localhost:8080 --key b0_def456...
```

### 4. Add Workers

```bash
b0 worker add reviewer --instructions "Senior code reviewer. Focus on correctness and edge cases."
b0 worker add security --instructions "Security engineer. Find vulnerabilities."
```

### 5. Delegate and Wait

```bash
b0 delegate reviewer "Review src/main.rs for correctness issues."
b0 delegate security "Check src/main.rs for security vulnerabilities."
b0 wait
```

That's it. Two workers analyze your code in parallel and report back.

## Using with Claude Code

Install the Box0 skill so Claude Code automatically delegates when appropriate:

```bash
b0 skill install claude-code
```

Now Claude Code knows about your workers. When you say "review this PR and check for security issues", Claude Code will automatically run `b0 delegate` to your workers instead of doing everything itself.

## Using with Codex

```bash
b0 skill install codex
```

This appends Box0 instructions to `~/.codex/AGENTS.md`. Codex will learn to delegate tasks to your workers.

## Using with Other Agents

```bash
b0 skill show
```

Prints the skill content to stdout. Paste it into whatever your agent uses for custom instructions.

## One-Off Tasks

Don't want to create a named worker? Use `worker temp`:

```bash
b0 worker temp "Summarize the top 5 differences between Rust and Go."
b0 wait
```

Creates a temporary worker, runs the task, and auto-cleans up.

## Multi-Machine

Run workers on different machines:

```bash
# Machine A: start server
b0 server --host 0.0.0.0

# Machine B: join as a worker node
b0 node join http://machine-a:8080 --name gpu-box --key <key>

# Machine A: add worker on the remote node
b0 worker add ml-agent --instructions "ML specialist." --node gpu-box
b0 delegate ml-agent "Analyze this dataset."
b0 wait
```

The task is routed to Machine B. Claude Code CLI runs there, using Machine B's authentication and compute.

## CLI Reference

```
b0 server [--host] [--port] [--db]       Start server
b0 login <url> --key <key>               Connect
b0 logout                                Disconnect
b0 reset                                 Clean slate (delete DB, config, skills)
b0 status                                Show connection info

b0 worker add <name> --instructions "..."  [--node <node>]
b0 worker ls
b0 worker info <name>
b0 worker update <name> --instructions "..."
b0 worker stop/start <name>
b0 worker logs <name>
b0 worker remove <name>
b0 worker temp "<task>"                  One-off (non-blocking)

b0 delegate <worker> "<task>"            Send task (non-blocking)
b0 delegate <worker>                     Read task from stdin
b0 wait                                  Collect results
b0 reply <thread-id> "<answer>"          Answer a worker's question

b0 node join <url> [--name] [--key]      Join as worker node
b0 node ls                               List nodes

b0 group create <name>                   Create group (admin)
b0 group invite <group> [--description]  Generate key (admin)
b0 group keys                            List keys
b0 group revoke <prefix>                 Revoke key (admin)

b0 skill install claude-code             Install for Claude Code
b0 skill install codex                   Install for Codex
b0 skill uninstall <agent>               Remove
b0 skill show                            Print to stdout
```

## How It Works

Workers are not long-running processes. When a task arrives, the node daemon spawns `claude --print --output-format json --system-prompt "<instructions>"` as a subprocess. The task is piped via stdin. When done, the result goes back through the server's inbox to whoever delegated it.

Workers use the machine's existing authentication (OAuth or API key). No special credential setup needed.

## License

MIT License. Copyright (c) RisingWave Labs.
