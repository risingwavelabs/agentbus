# Stream0

Stream0 is a messaging layer for agents.

It gives each agent an inbox and a thread-based message history, so one agent can delegate work to other agents, continue the discussion on the same task thread, and gather results back into one place.

## What Is Stream0?

Stream0 is not a model, and it is not an agent runtime. It is the transport layer behind multi-agent workflows.

Use it when one agent needs to work with other agents on the same task:

- delegate subtasks
- split work across specialists
- discuss an intermediate result
- ask follow-up questions
- collect final outcomes on one thread

## What Problem Does It Solve?

Calling another program over HTTP is easy. Coordinating work across multiple agents is harder.

Once more than one agent is involved in the same task, the coordination itself becomes a problem:

- one agent needs to hand work to another
- one task needs to be split across multiple agents
- a worker may need clarification before it can continue
- intermediate discussion needs to stay attached to the task
- results need to come back to the original requester
- the whole exchange needs a clear terminal state

Stream0 keeps that workflow simple:

- each agent has an inbox
- each task has a `thread_id`
- messages stay attached to that thread
- the task progresses through typed messages such as `request`, `question`, `answer`, `done`, and `failed`

The point is not just to move one message. The point is to keep one task moving until it is finished.

## A Concrete Example

The simplest useful mental model is:

- a user talks to one primary agent
- that primary agent uses Stream0 behind the scenes
- specialist agents do part of the work and report back
- the user gets one final result

For example:

- the user asks a primary agent to write a recommendation memo
- the primary agent asks a research worker for market context
- the primary agent asks a critic worker to challenge the positioning
- the critic worker asks a clarification question
- the primary agent answers
- both workers return results
- the primary agent gives one final recommendation back to the user

The flow looks like this:

```text
User → primary-agent           : "Write a recommendation memo"
primary-agent → research-worker: request
primary-agent → critic-worker  : request
critic-worker → primary-agent  : question
primary-agent → critic-worker  : answer
research-worker → primary-agent: done
critic-worker → primary-agent  : done
primary-agent → User           : final result
```

That is the core Stream0 pattern: the user talks to one agent, and that agent coordinates other agents automatically through a shared task thread.

## Typical Use Cases

- **Delegation**: one agent hands a task to another and waits for the result
- **Parallel subtasks**: one agent fans work out to multiple specialist agents and gathers outputs
- **Discussion**: agents compare alternatives or challenge assumptions before proceeding
- **Clarification loops**: a worker asks follow-up questions on the same task thread
- **Interactive + background coordination**: an interactive agent stays in the loop while background workers handle longer-running work

## What Stream0 Provides

- agent addressing
- inbox persistence
- thread-scoped task history
- typed task-state messages

## What Stream0 Does Not Provide

Stream0 is not an orchestration engine and not an execution runtime.

It does not:

- execute your model
- decide which agents to call
- schedule your worker
- manage tools or memory
- replace direct RPC for simple synchronous calls

Those decisions still belong to your primary agent or your application logic. Stream0 provides the messaging primitives underneath.

## Inbox Is Not Execution

Registering an agent in Stream0 gives it an inbox. It does not automatically make that agent run.

To process messages automatically, you still need an activation mechanism:

- a polling worker loop
- a webhook
- a [Channel integration](docs/tutorial-channel.md)
- an external scheduler

This is intentional. Stream0 handles message delivery and thread history. Your agents still decide when and how to act.

## Quick Start

### Start Stream0

```bash
cargo build --release
STREAM0_DB_PATH=/tmp/stream0-demo.db ./target/release/stream0
```

### Verify the server is up

```bash
curl http://localhost:8080/health
```

Expected response:

```json
{"status":"healthy","version":"0.2.0-rust"}
```

### Run the auto-coordination demo

```bash
./demo.sh
```

The demo shows the full pattern:

- the user gives one goal to a primary agent
- the primary agent fans the work out to two specialist workers
- one worker asks a clarification question
- both workers return `done`
- the primary agent returns one final result to the user
- the full thread history is printed at the end

## How It Works

### 1. Register agents

Each participating agent registers once and gets an inbox.

```bash
curl -X POST http://localhost:8080/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "primary-agent"}'

curl -X POST http://localhost:8080/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "research-worker"}'

curl -X POST http://localhost:8080/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "critic-worker"}'
```

### 2. Send work to another agent

```bash
curl -X POST http://localhost:8080/agents/research-worker/inbox \
  -H "Content-Type: application/json" \
  -d '{
    "thread_id": "strategy-memo-1",
    "from": "primary-agent",
    "type": "request",
    "content": {
      "task": "Gather market context for Stream0."
    }
  }'
```

### 3. Read inbox messages

```bash
curl "http://localhost:8080/agents/research-worker/inbox?status=unread&thread_id=strategy-memo-1"
```

### 4. Continue the same thread

Workers can respond with `done`, `failed`, or `question`. The primary agent can answer on the same `thread_id`.

### 5. Retrieve the full thread history

```bash
curl "http://localhost:8080/threads/strategy-memo-1/messages"
```

## Why Not Just Use HTTP?

Because the problem is usually not "how do I make one request?" The problem is "how does one agent hand off part of a task, keep the discussion attached to that task, and gather the results back later?"

Use direct HTTP when:

- both sides are online
- the interaction is a single synchronous request/response
- there is no threaded discussion or follow-up

Use Stream0 when:

- one primary agent needs to coordinate other agents
- tasks may take time
- work may span multiple messages
- a worker may need clarification before continuing
- you want the full task history on one thread

## Claude Code Channel Integration

If your primary agent is Claude Code, the recommended integration is a Channel plugin. Messages sent to your Claude Code inbox appear in the session automatically, so Claude Code can participate in Stream0 workflows without manual polling.

See the [Channel tutorial](docs/tutorial-channel.md) for the full setup guide.

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
