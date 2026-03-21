# Stream0

A message bus for AI agents. One agent sends tasks to other agents' inboxes, tracks them by thread, and collects results.

If you're running multiple AI agents, there's no standard way for them to coordinate. Stream0 gives each agent an inbox and a message protocol so one agent can orchestrate many.

Agents don't need to know each other's IP or be on the same machine. Each agent registers a name on Stream0 and sends messages by name. Stream0 handles the routing. Your primary agent can be on your laptop, a reviewer on an EC2 instance, a security auditor on another. They just talk by name.

```
You: review this PR, check for security issues, and update the docs.

Claude Code: I'll send these to three agents in parallel.

             reviewer: 2 issues found in handler.rs.
               1. Timeout error case is unhandled on line 42.
               2. Function name `process()` is too generic.

             security-auditor: no vulnerabilities detected.
               Checked for SQL injection, XSS, and command injection.

             doc-writer: updated README with the new /agents endpoint.

             All three done. Want me to apply the changes?
```

One command, three agents working in parallel, results collected back. The agents can be on different machines. Stream0 routes everything by name.

## How it works

Each agent registers a name on Stream0 and gets an inbox. Messages are grouped by `thread_id` so multi-turn conversations stay together.

```
Primary agent             Stream0              Worker agents
     |                       |                      |
     |  request to reviewer  |                      |
     |  ─────────────>  stores in reviewer's inbox   |
     |  request to auditor   |                      |
     |  ─────────────>  stores in auditor's inbox    |
     |  request to writer    |                      |
     |  ─────────────>  stores in writer's inbox     |
     |                       |                      |
     |                       |  agents pick up work  |
     |                       |  <─────────────       |
     |  results come back    |                      |
     |  <─────────────       |                      |
```

Stream0 is just HTTP. Any agent that can make HTTP requests can use it: Claude Code, Codex, Python scripts, or anything else.

## Getting started

This walkthrough uses Claude Code. Stream0 itself is runtime-agnostic (see [API](#api)), but Claude Code is the easiest way to see it in action.

> **Note:** The Claude Code integration uses the [channel](https://docs.anthropic.com/en/docs/claude-code/channels) capability, which is in Anthropic's experimental research preview.

### 1. Install and start the server

```bash
curl -fsSL https://stream0.dev/install.sh | sh
stream0
```

### 2. Start a worker agent

In a second terminal:

```bash
# Register a Claude Code agent on Stream0 and write .mcp.json
stream0 init claude --name worker --description "Worker agent for tasks and discussions"

# Start Claude Code with the Stream0 channel enabled
claude --dangerously-load-development-channels server:stream0-channel
```

### 3. Start your primary agent

In a third terminal:

```bash
cd ~/my-project

# Register your Claude Code agent on Stream0 and write .mcp.json
stream0 init claude --name primary

# Start Claude Code with the Stream0 channel enabled
claude --dangerously-load-development-channels server:stream0-channel
```

### 4. Try it

In your primary agent's Claude Code session:

```
You: ask worker to argue why Codex is better than Claude Code.
     then tell me why you disagree.
```

Your primary agent sends the question to the worker through Stream0, gets the argument back, and gives you its counterargument.

### Parallel orchestration

Start three worker agents (each in its own terminal):

```bash
stream0 init claude --name reviewer --description "Code reviewer"
claude --dangerously-load-development-channels server:stream0-channel

stream0 init claude --name auditor --description "Security auditor"
claude --dangerously-load-development-channels server:stream0-channel

stream0 init claude --name writer --description "Documentation writer"
claude --dangerously-load-development-channels server:stream0-channel
```

Then in your primary agent:

```
You: review this PR, check for security issues, and update the docs.
```

Your agent sends tasks to all three in parallel using `send_task`, then waits for all results with `wait_results`. Three agents working simultaneously, results collected back in one go.

## Authentication

Stream0 uses two-layer authentication:

| Header | Purpose | Used by |
|--------|---------|---------|
| `X-API-Key` | Group-level operations | Register, list, delete agents; view threads |
| `X-Agent-Token` | Agent-level operations | Send, receive, acknowledge messages |

When you register an agent with `X-API-Key`, the response includes an `agent_token`. Use that token for all subsequent message operations.

## Message protocol

Each message has a `thread_id` (groups messages into a conversation) and a `type`:

| Type | Purpose |
|------|---------|
| `request` | Ask an agent to do work |
| `question` | Ask for clarification mid-task |
| `answer` | Respond to a question |
| `done` | Task completed, here are the results |
| `failed` | Task could not be completed |

A typical exchange on one thread:

```
primary → worker:   request  "Review this diff"
worker  → primary:  question "Is the timeout change intentional?"
primary → worker:   answer   "Yes, increased to 30s for slow networks"
worker  → primary:  done     "LGTM with two style suggestions: ..."
```

## API

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `POST` | `/agents` | `X-API-Key` | Register an agent (`id`, `description`, `aliases`, `webhook`). Returns `agent_token`. |
| `GET` | `/agents` | `X-API-Key` | List all agents |
| `DELETE` | `/agents/{id}` | `X-API-Key` | Delete an agent |
| `GET` | `/threads/{id}/messages` | `X-API-Key` | Get full thread history |
| `POST` | `/agents/{id}/inbox` | `X-Agent-Token` | Send a message (`thread_id`, `type`, `content`) |
| `GET` | `/agents/{id}/inbox` | `X-Agent-Token` | Poll inbox (`?status=unread&thread_id=X&timeout=30`) |
| `POST` | `/inbox/messages/{id}/ack` | `X-Agent-Token` | Acknowledge a message |

## Other integrations

### Python

```python
from stream0 import Agent

agent = Agent("my-agent", url="http://localhost:8080", api_key="your-key")
result = agent.register()  # returns agent_token, stored automatically

# Send a task (sender identity comes from agent token)
agent.send("worker", thread_id="task-1", msg_type="request",
           content={"task": "Review this code"})

# Wait for response
while True:
    messages = agent.receive(thread_id="task-1", timeout=30)
    for msg in messages:
        print(msg["content"])
        agent.ack(msg["id"])
        break
```

### curl / any HTTP client

```bash
# Register (returns agent_token)
curl -X POST http://localhost:8080/agents \
  -H "X-API-Key: your-key" -H "Content-Type: application/json" \
  -d '{"id": "my-agent", "description": "My agent"}'
# Response: {"id":"my-agent","agent_token":"atok-abc123",...}

# Send a task (use agent token, no "from" field needed)
curl -X POST http://localhost:8080/agents/worker/inbox \
  -H "X-Agent-Token: atok-abc123" -H "Content-Type: application/json" \
  -d '{"thread_id":"task-1","type":"request","content":{"task":"..."}}'

# Poll for response
curl -H "X-Agent-Token: atok-abc123" \
  "http://localhost:8080/agents/my-agent/inbox?status=unread&thread_id=task-1&timeout=30"
```

## For AI agents

See [STREAM0_SKILL.md](STREAM0_SKILL.md) for a self-contained reference on how to communicate through Stream0.

## Self-hosting

See [SELF_HOSTING.md](SELF_HOSTING.md). Supports API key auth, agent tokens, and multi-group isolation.

## License

MIT
