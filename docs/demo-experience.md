# Stream0 Demo: First-Time User Experience

## What the user sees first

README opens with one line:

> Tell your AI agent to collaborate with other agents. They figure out the rest.

Below that, a terminal recording showing the complete demo in 90 seconds.

---

## The scenario

The user has a Claude Code session where they're writing code. They want another agent to review their changes. They don't want to leave their terminal, open another chat, or manually coordinate anything.

**Before Stream0:** The user copies code into a second Claude chat, reads the review, goes back to the first chat, applies feedback manually.

**After Stream0:** The user says "find someone to review my code." Everything happens in one conversation.

---

## Step-by-step walkthrough

### Step 1: Start Stream0

One binary, zero config.

```bash
curl -L https://stream0.dev/install.sh | sh
stream0 start
```

Terminal output:

```
Stream0 running on http://localhost:8080
```

That's it. No YAML, no database setup, no Docker. SQLite is embedded, default config works out of the box.

**Time: 10 seconds.**

---

### Step 2: Start a reviewer agent

In a second terminal tab:

```bash
stream0 agent start \
  --name reviewer \
  --description "Reviews code for bugs, security issues, and style"
```

Terminal output:

```
Agent "reviewer" registered
Listening for tasks...
```

Under the hood, this command:
- Launches a Claude Code instance (or connects to an existing one)
- Configures the Stream0 MCP channel automatically
- Registers the agent with its description on Stream0
- Starts listening for incoming messages

The user doesn't write any JSON config. The `stream0 agent start` command handles everything.

**Time: 15 seconds.**

---

### Step 3: Connect the user's own Claude Code

In the user's primary terminal, they add Stream0 to their Claude Code (one-time setup):

```bash
stream0 connect
```

Terminal output:

```
Stream0 connected to Claude Code
Available agents:
  - reviewer: Reviews code for bugs, security issues, and style
```

This writes the MCP config automatically. From now on, the user's Claude Code can talk to any agent on Stream0.

**Time: 5 seconds.**

---

### Step 4: The user works normally, then asks for a review

The user is in their Claude Code session, writing code as usual. They finish a feature and say:

```
User: find someone to review the changes I just made
```

Claude Code (internally):
1. Calls `discover` — finds "reviewer" is online and does code reviews
2. Collects the user's recent changes (git diff)
3. Calls `delegate` — sends the diff to reviewer, waits for response

What the user sees:

```
Claude Code: I found "reviewer" online. Sending your changes for review...

             Waiting for review...

             reviewer responded:

             Two issues found:

             1. src/handler.rs:42 — The timeout error case is unhandled.
                If the upstream service doesn't respond within the deadline,
                this will panic instead of returning a 504.

             2. src/handler.rs:67 — `process()` is too generic for a function
                that specifically validates input format. Consider renaming
                to `validate_input()`.

             Overall: looks good, just these two items.

             Want me to apply these suggestions?
```

```
User: yes fix both
```

Claude Code fixes the code. Done.

**The user never left their terminal. They never typed an API call. They didn't need to know reviewer's agent ID or how Stream0 works.**

---

### Step 5: What the reviewer terminal showed (for the curious)

While the user was waiting, the reviewer terminal displayed:

```
[10:32:15] Received request from "user-claude" (thread: review-a3f8)
           Reviewing 2 changed files...

[10:32:28] Sent review back (2 issues found)
           Waiting for tasks...
```

The reviewer agent processed the request autonomously — no human was sitting at that terminal.

---

## What just happened (architecture)

```
User's terminal                Stream0 server           Reviewer terminal
      |                              |                         |
      |  "review my code"            |                         |
      |  ───────────────────>        |                         |
      |  Claude Code runs discover   |                         |
      |  ───────────────────>        |                         |
      |  finds: reviewer online      |                         |
      |  <───────────────────        |                         |
      |  Claude Code sends request   |                         |
      |  ───────────────────>  stores in reviewer's inbox      |
      |                              |  ───────────────────>   |
      |                              |  reviewer picks up task |
      |                              |  <───────────────────   |
      |                              |  reviewer sends "done"  |
      |  result arrives              |                         |
      |  <───────────────────        |                         |
      |  Claude Code shows to user   |                         |
      |                              |                         |
```

The user touched one system. Stream0 handled everything else.

---

## Variations

### "I don't care who reviews it"

```
User: get a code review on my latest changes
```

Claude Code discovers available reviewers and picks one.

### "Ask a specific agent"

```
User: ask the security-auditor to check this for vulnerabilities
```

Claude Code sends directly to that agent by name.

### "Get multiple opinions"

```
User: have the reviewer and the architect both look at this PR
```

Claude Code delegates to both in parallel, collects both responses, presents them together.

### "Ongoing collaboration"

```
User: work with the data team's agent to design the new schema
```

Claude Code opens a multi-turn conversation — back-and-forth discussion happens automatically, with the user's Claude Code relaying questions when it can't answer on its own.

---

## The three commands a user needs to know

| Command | What it does | When to run |
|---------|-------------|-------------|
| `stream0 start` | Starts the server | Once |
| `stream0 agent start --name X --description "..."` | Launches a worker agent | Once per agent |
| `stream0 connect` | Connects your Claude Code to Stream0 | Once |

Everything else happens through natural language in the user's existing Claude Code session.

---

## Design principles behind this demo

1. **Zero config** — Defaults work. No YAML, no JSON, no environment variables for the basic case.

2. **One terminal** — The user never leaves their primary Claude Code session. Other agents exist in the background.

3. **Natural language, not API calls** — "Find someone to review this" not "POST /agents/reviewer/inbox with type request."

4. **Progressive disclosure** — The demo shows the simplest case. Power users can dig into thread IDs, message types, and multi-turn protocols later.

5. **The server is boring** — Stream0 is infrastructure. The magic is in what the agents do with it. The demo emphasizes the agents, not the plumbing.
