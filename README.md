# Stream0

Stream0 is a communication layer for agents.

It gives each agent an inbox, and lets a task move back and forth between agents on the same thread until it is finished.

## What Is Stream0?

Stream0 is not a model, and it is not an agent runtime.

It is the layer that lets agents collaborate.

Each agent gets:
- an identity
- an inbox
- a thread-based message history

With Stream0, agents do not just send one-off messages. They move a task forward together.

## What Problem Does It Solve?

Calling another program over HTTP is easy.

What is hard is coordinating a task across agents:

- the other side may not be online right now
- the task may take time
- the worker may need clarification halfway through
- the whole exchange needs to stay tied to one task
- you need to know whether the task finished or failed

Stream0 solves this by turning agent collaboration into a simple workflow:

`request → question → answer → done`

Instead of thinking in terms of isolated calls, you think in terms of a task thread.

## A Concrete Example

The easiest way to understand Stream0 is this:

- `Claude Code` is the coordinator
- `worker.py` is a background code-review worker

Claude Code receives a request to review a PR. Instead of doing everything itself, it sends the review task to `worker.py`.

`worker.py` starts reviewing the code. Halfway through, it finds something unclear, so it asks a question back to Claude Code. Claude Code answers. Then `worker.py` finishes the review and returns the result.

The flow looks like this:

```text
Claude Code → worker.py : request    "Review PR #42"
worker.py → Claude Code : question   "Line 42 shadows a variable. Intentional?"
Claude Code → worker.py : answer     "Yes, it's a test override."
worker.py → Claude Code : done       "LGTM. Approved."
```

This is the core Stream0 workflow.

It is not just "send a message." It is "hand off a task, keep the task moving, and finish it on the same thread."

## How It Works

### 1. Register both sides

Each side registers once and gets its own inbox.

```bash
curl -X POST http://localhost:8080/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "claude-code"}'

curl -X POST http://localhost:8080/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "review-worker"}'
```

### 2. Claude Code sends a task

```bash
curl -X POST http://localhost:8080/agents/review-worker/inbox \
  -H "Content-Type: application/json" \
  -d '{
    "thread_id": "review-pr-42",
    "from": "claude-code",
    "type": "request",
    "content": {
      "instruction": "Review this PR",
      "pr_url": "https://github.com/acme/app/pull/42"
    }
  }'
```

The message includes who it is for, who sent it, the task content, and a `thread_id`.

### 3. worker.py processes the task

worker.py polls its inbox, picks up the request, and starts work.

```bash
curl "http://localhost:8080/agents/review-worker/inbox?status=unread"
```

At that point it can do one of three things:

- send `done` if the task is complete
- send `failed` if it cannot complete the task
- send `question` if it needs clarification

### 4. worker.py asks a question

```bash
curl -X POST http://localhost:8080/agents/claude-code/inbox \
  -H "Content-Type: application/json" \
  -d '{
    "thread_id": "review-pr-42",
    "from": "review-worker",
    "type": "question",
    "content": {
      "question": "Is the shadowed variable in auth.rs intentional?"
    }
  }'
```

### 5. Claude Code answers

```bash
curl -X POST http://localhost:8080/agents/review-worker/inbox \
  -H "Content-Type: application/json" \
  -d '{
    "thread_id": "review-pr-42",
    "from": "claude-code",
    "type": "answer",
    "content": {"answer": "Intentional — it is a test override."}
  }'
```

### 6. worker.py completes the task

```bash
curl -X POST http://localhost:8080/agents/claude-code/inbox \
  -H "Content-Type: application/json" \
  -d '{
    "thread_id": "review-pr-42",
    "from": "review-worker",
    "type": "done",
    "content": {
      "approved": true,
      "summary": "Review complete. Variable shadow is intentional."
    }
  }'
```

### 7. View the full thread

```bash
curl "http://localhost:8080/threads/review-pr-42/messages"
```

Returns all 4 messages in order: `request → question → answer → done`.

That is the basic Stream0 pattern. The thread keeps moving until it reaches `done` or `failed`.

## Why Not Just Use HTTP?

Because the real problem is usually not "how do I make one request?"

The real problem is:

- how do I hand work to another agent
- let it process later if needed
- let it ask questions midway
- keep the whole exchange tied to one task
- get a clear final result

HTTP is great for direct request/response. Stream0 is for task-oriented agent coordination.

## Inbox Is Not Execution

Registering an agent in Stream0 gives it an inbox. It does not automatically make that agent run.

For an agent to actually process incoming work, you still need some activation mechanism:

- a [Channel integration](docs/tutorial-channel.md) (recommended for Claude Code)
- a [polling loop](mcp/worker.py) (for background workers)
- a webhook
- an external scheduler

## Claude Code Channel Integration

The recommended way to connect Claude Code to Stream0 is through a Channel plugin. With a Channel, messages sent to your Claude Code's inbox automatically appear in your session. You do not need to poll or check manually.

See the [Channel tutorial](docs/tutorial-channel.md) for the full setup guide.

```bash
# Start Claude Code with Stream0 channel
claude --channels server:stream0-channel
```

From that moment, any agent can send tasks to your Claude Code and it will receive them automatically.

## Quick Start

### Start Stream0

```bash
cargo build --release
./target/release/stream0
```

By default, Stream0 runs on `http://localhost:8080`.

### Run the demo

```bash
./demo.sh
```

The demo registers two agents, walks through a complete code review with a mid-task question, and shows the full conversation history.

## Message Types

| Type | Purpose |
|------|---------|
| `request` | Ask an agent to do work |
| `question` | Need clarification mid-task |
| `answer` | Reply to a question |
| `done` | Task completed successfully |
| `failed` | Task could not be completed |
| `message` | General-purpose message |

## API Reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/agents` | Register an agent (with optional `aliases` and `webhook`) |
| `GET` | `/agents` | List all agents (includes `aliases` and `last_seen`) |
| `DELETE` | `/agents/{id}` | Delete an agent |
| `POST` | `/agents/{id}/inbox` | Send a message |
| `GET` | `/agents/{id}/inbox` | Read inbox (`?status=unread&thread_id=X&timeout=10`) |
| `POST` | `/inbox/messages/{id}/ack` | Mark a message as read |
| `GET` | `/threads/{thread_id}/messages` | Full conversation history |
| `GET` | `/health` | Health check |

## Deploying to Production

See [SELF_HOSTING.md](SELF_HOSTING.md) for the full guide. The short version:

```bash
cargo build --release
./target/release/stream0 --config stream0.yaml
```

Stream0 supports API key authentication and multi-tenant isolation. Multiple teams can share one instance without seeing each other's data.

## For AI Agents

If you are an AI agent reading this, see [STREAM0_SKILL.md](STREAM0_SKILL.md) — a self-contained reference with everything you need to start communicating through Stream0.

## License

MIT
