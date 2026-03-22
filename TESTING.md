# Box0 Manual Testing Guide

## Prerequisites

- Rust toolchain installed
- Claude Code CLI installed and authenticated (`claude --version`)

```bash
cd box0
cargo build --release
export PATH="$PWD/target/release:$PATH"
b0 --version
```

---

## Test 1: Server start + first use

```bash
b0 server
# Expected: prints admin key + user ID
# Admin has personal group "admin", auto-logged in on server machine

# In another terminal (same machine):
b0 worker add --group admin reviewer --instructions "Answer in one word."
b0 delegate --group admin reviewer "Capital of France?"
b0 wait
# Expected: reviewer done: Paris

b0 worker remove --group admin reviewer
```

---

## Test 2: Invite user + shared group

```bash
# As admin:
b0 invite alice
# Expected: prints alice's key + user ID

b0 group create dev-team
b0 group add-member dev-team <alice-user-id>
b0 group ls
# Expected: admin, dev-team

# As alice (from another terminal or machine):
b0 login http://localhost:8080 --key <alice-key>
b0 group ls
# Expected: alice (personal), dev-team

b0 worker add --group dev-team reviewer --instructions "Be brief."
b0 worker ls --group dev-team
# Expected: reviewer

# Alice cannot see admin's personal group workers:
b0 worker ls --group admin
# Expected: error (not a member)
```

---

## Test 3: Worker ownership

```bash
# Alice creates a worker in dev-team
b0 login http://localhost:8080 --key <alice-key>
b0 worker add --group dev-team alice-worker --instructions "x"

# Admin tries to remove alice's worker
b0 login http://localhost:8080 --key <admin-key>
b0 worker remove --group dev-team alice-worker
# Expected: permission denied

# Alice can remove her own
b0 login http://localhost:8080 --key <alice-key>
b0 worker remove --group dev-team alice-worker
# Expected: success
```

---

## Test 4: Node ownership

```bash
# Alice registers her GPU machine
b0 node join http://localhost:8080 --name alice-gpu --key <alice-key>

# Alice can deploy workers to her node
b0 worker add --group dev-team ml-agent --instructions "ML." --node alice-gpu

# Admin cannot deploy workers to alice's node
b0 login http://localhost:8080 --key <admin-key>
b0 worker add --group dev-team hacked --instructions "x" --node alice-gpu
# Expected: error (you don't own this node)
```

---

## Test 5: Worker temp + skill install

```bash
b0 worker temp --group admin "What is 2+2? Just the number."
b0 wait
# Expected: done: 4, temp worker auto-cleaned

b0 skill install claude-code
ls ~/.claude/skills/b0/SKILL.md
# Expected: exists

b0 skill install codex
head -1 ~/.codex/AGENTS.md
# Expected: box0 marker

b0 skill uninstall claude-code
b0 skill uninstall codex
```

---

## Test 6: Reset

```bash
b0 reset
# Expected: removes DB, config, skills
ls b0.db 2>&1
# Expected: not found
```

---

## Cleanup

```bash
rm -rf ~/.b0/ ~/.claude/skills/b0 b0.db
```
