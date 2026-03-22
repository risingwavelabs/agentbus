# Box0 Manual Testing Guide

## Prerequisites

- Rust toolchain installed
- Claude Code CLI installed and authenticated (run `claude --version` to verify)
- For multi-machine tests: two machines that can reach each other over the network

Build first:

```bash
cd box0
cargo build --release
export PATH="$PWD/target/release:$PATH"
```

Verify:

```bash
b0 --version
b0 --help
```

---

## Test 1: Server Bootstrap + Login

**Goal**: Verify server starts, generates admin key, and login works.

Terminal 1 - start server:
```bash
b0 server
# Expected:
#   Admin key: b0_<long-key>
#   Save this key. Use it to login:
#   b0 login http://127.0.0.1:8080 --key b0_<long-key>
```

Terminal 2 - login:
```bash
# Login with the admin key printed above
b0 login http://localhost:8080 --key <admin-key>
# Expected: "Connected", "Login complete."
# Expected: "To install agent skill: b0 skill install claude-code  (or: codex)"

# Install skill (separate step)
b0 skill install claude-code
# Expected: "Skill installed for Claude Code (~/.claude/skills/b0/SKILL.md)"

ls ~/.claude/skills/b0/SKILL.md
cat ~/.claude/skills/b0/SKILL.md | head -5
# Expected: YAML frontmatter with name: bh

# Or for Codex:
b0 skill install codex
# Expected: "Skill installed for Codex (~/.codex/AGENTS.md)"

# Verify config
cat ~/.b0/config.toml
# Expected: server_url, lead_id, api_key
```

### What to Check

- [ ] Server prints admin key on first start
- [ ] Second start does NOT print a new key (reuses existing)
- [ ] `b0 login --key` succeeds
- [ ] Skill file created at `~/.claude/skills/b0/SKILL.md`
- [ ] Config saved at `~/.b0/config.toml` with api_key

---

## Test 2: Groups + Access Control

**Goal**: Verify group isolation and admin-only operations.

```bash
# As admin:
b0 group create frontend
b0 group create ml-team
b0 group ls
# Expected: 2 groups

# Invite members
b0 group invite frontend --description "alice"
# Expected: prints key for alice. SAVE IT.

b0 group invite ml-team --description "bob"
# Expected: prints key for bob. SAVE IT.

# List all keys (admin sees all)
b0 group keys
# Expected: 3 keys (admin + alice + bob) with role and group columns

# Login as alice
b0 login http://localhost:8080 --key <alice-key>
b0 worker add reviewer --instructions "Review code."
b0 worker ls
# Expected: only sees reviewer

# Login as bob
b0 login http://localhost:8080 --key <bob-key>
b0 worker add ml-agent --instructions "ML tasks."
b0 worker ls
# Expected: only sees ml-agent (NOT reviewer)

# Non-admin cannot create groups
b0 group create hacked
# Expected: "Error: admin key required"
```

### What to Check

- [ ] Groups created successfully
- [ ] Group keys scoped to their group
- [ ] Alice only sees frontend workers
- [ ] Bob only sees ml-team workers
- [ ] Non-admin cannot create groups or revoke keys

---

## Test 3: Basic Worker Flow

**Goal**: Verify delegate + wait end-to-end.

```bash
b0 login http://localhost:8080 --key <group-key>
b0 worker add reviewer --instructions "Be concise - max 1 sentence."
b0 delegate reviewer "Is Rust a good language?"
# Expected: prints thread-id immediately (non-blocking)

b0 wait
# Expected: blocks, then prints result

b0 worker remove reviewer
```

---

## Test 4: Worker Temp

**Goal**: Verify one-off tasks work (non-blocking).

```bash
b0 worker temp "What is 2+2? Just the number."
# Expected: prints thread-id immediately

b0 wait
# Expected: prints result, temp worker auto-cleaned

b0 worker ls
# Expected: no workers (temp worker removed)
```

---

## Test 5: Delegate from Stdin

**Goal**: Verify large content can be piped via stdin.

```bash
echo "List the first 5 prime numbers." | b0 delegate reviewer
# Expected: prints thread-id

b0 wait
# Expected: prints result
```

---

## Test 6: Worker Lifecycle

**Goal**: Verify info, update, stop, start, logs.

```bash
b0 worker add test-worker --instructions "Be brief."
b0 worker info test-worker
# Expected: shows name, node, status, registered_by, instructions

b0 worker update test-worker --instructions "Be very brief."
b0 worker info test-worker | grep Instructions
# Expected: "Be very brief."

b0 worker stop test-worker
b0 worker ls
# Expected: status = stopped

b0 worker start test-worker
b0 worker ls
# Expected: status = active

# Delegate and check logs
b0 delegate test-worker "Say hello"
b0 wait
b0 worker logs test-worker
# Expected: shows request + done messages

b0 worker remove test-worker
```

---

## Test 7: Worker Ownership

**Goal**: Verify only creator can modify/delete workers.

Requires two different group keys (alice and bob in the same group, or admin + member).

```bash
# Login as alice, create a worker
b0 login http://localhost:8080 --key <alice-key>
b0 worker add alice-worker --instructions "x"

# Login as bob, try to remove it
b0 login http://localhost:8080 --key <bob-key>
b0 worker remove alice-worker
# Expected: "Error: permission denied: worker was created by someone else"

b0 worker stop alice-worker
# Expected: "Error: permission denied"

# Bob CAN see and delegate to it
b0 worker ls
# Expected: alice-worker is listed

# Alice can remove her own worker
b0 login http://localhost:8080 --key <alice-key>
b0 worker remove alice-worker
# Expected: success
```

---

## Test 8: Multi-Machine

**Goal**: Verify remote nodes.

Machine A - start server:
```bash
b0 server --host 0.0.0.0 --port 8080
# Save the admin key

b0 login http://localhost:8080 --key <admin-key>
b0 group create team
b0 group invite team --description "node-key"
# Save the group key
```

Machine B - join as node:
```bash
b0 node join http://<machine-a-ip>:8080 --name remote-box --key <group-key>
# Expected: "Joining as node" + daemon starts
```

Machine A - use remote node:
```bash
b0 login http://localhost:8080 --key <group-key>
b0 node ls
# Expected: local + remote-box

b0 worker add remote-w --instructions "Be brief." --node remote-box
b0 delegate remote-w "What is 1+1?"
b0 wait
# Expected: result comes back from remote node
```

### Troubleshooting

- Machine A must bind to `0.0.0.0` (not `127.0.0.1`)
- Check firewall on port 8080
- Verify with `curl http://<ip>:8080/health` from Machine B
- Claude Code CLI must be installed on Machine B

---

## Test 9: Edge Cases

```bash
# Delegate to nonexistent worker
b0 delegate nonexistent "hello"
# Expected: error message

# Wait with no pending tasks
b0 wait
# Expected: "No pending tasks."

# Login without key
b0 login http://localhost:8080
# Expected: works (health check is public), but subsequent commands fail

# Server not running
b0 worker ls
# Expected: connection error
```

---

## Cleanup

```bash
# Stop server (Ctrl+C)
# Stop remote daemons (Ctrl+C)

rm -rf ~/.b0/
rm -rf ~/.claude/skills/b0
rm -f b0.db
```
