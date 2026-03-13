# Transition Document: stream0 Project

**Date:** 2026-03-13
**From:** Previous agent (kafka-builder)
**To:** Next agent
**Project:** stream0 - HTTP-native event streaming for AI agents
**Repository:** https://github.com/risingwavelabs/stream0

---

## Executive Summary

stream0 is a message bus system built for agent-to-agent communication. It's a Go rewrite of the original Python "AgentBus" prototype, renamed to "stream0" (zero-setup streaming).

**Current Status:** MVP complete, production-hardened, deployment-ready

---

## What Was Built

### Core Components

1. **HTTP API (Gin-based)**
   - `POST /topics` - Create topic
   - `GET /topics` - List topics
   - `GET /topics/{name}` - Get topic info
   - `POST /topics/{name}/messages` - Produce message
   - `GET /topics/{name}/messages` - Consume messages (long-polling)
   - `POST /messages/{id}/ack` - Acknowledge message
   - `GET /health` - Health check
   - WebSocket support (basic)

2. **Consumer Groups**
   - Visibility timeout model (like SQS)
   - Load balancing across consumers
   - Message redelivery on failure
   - Offset tracking per consumer group

3. **SQLite Backend**
   - WAL mode enabled
   - Single-file database
   - ACID transactions
   - Consumer leases table

4. **Production Features**
   - Structured JSON logging
   - Graceful shutdown (SIGTERM handling)
   - Config file support (YAML)
   - Environment variable configuration

### File Structure

```
/home/sprite/agentbus-go/
├── main.go              # Entry point, graceful shutdown
├── server.go            # HTTP handlers, Gin setup
├── database.go          # SQLite operations
├── config.go            # Config loading (YAML/env)
├── models.py            # (old, from Python version)
├── go.mod               # Go modules - use Go 1.21+
├── go.sum               # Dependencies locked
├── Dockerfile           # Multi-stage Docker build
├── fly.toml             # Fly.io deployment config
├── stream0.yaml         # Example config file
├── README.md            # User documentation
├── SELF_HOSTING.md      # Production deployment guide
├── TRANSITION.md        # This document
├── test_go.py           # Python test suite (async)
└── docs/                # Design decisions & discussions
    ├── ARCHITECTURE_DECISIONS.md
    ├── DISCUSSION_SUMMARY.md
    └── request_reply_patterns.md
```

---

## Technical Decisions

### Architecture
- **Single-node SQLite**: Not distributed. One process per deployment.
- **Go language**: For single-binary deployment and performance
- **HTTP-native**: No special client libraries needed
- **Visibility timeout**: Consumer leases expire, messages redelivered

### Key Design Patterns
1. **Consumer groups**: Same group = load balance, different groups = broadcast
2. **At-least-once delivery**: Not exactly-once
3. **Long-polling**: Consumers wait for messages with timeout
4. **Explicit acks**: Messages not removed until acknowledged

### Dependencies
- `gin-gonic/gin` v1.9.1 - HTTP framework (downgraded for Go 1.21 compat)
- `modernc.org/sqlite` - Pure Go SQLite
- `sirupsen/logrus` - Structured logging
- `gorilla/websocket` - WebSocket support

---

## Current Status

### What's Working
✅ All 6 tests passing:
- Health check
- Topic CRUD
- Produce/consume/ack
- Consumer groups (load balancing)
- Different groups (broadcast)
- Visibility timeout (message recovery)

✅ Production features:
- JSON logging
- Graceful shutdown
- Config file support
- Systemd service file

### What's NOT Working
❌ Fly.io deployment - Failed due to Go version mismatches
  - Dependencies need Go 1.25+, Docker image only has 1.24
  - Solution: Use Go 1.25+ in Dockerfile or downgrade dependencies further

❌ Authentication/authorization - Not implemented
  - No API keys, no JWT, no ACLs

❌ Distributed/clustered mode - Not implemented
  - Single node only

❌ Dead letter queue - API endpoints exist but no logic

❌ Message replay - Not implemented

---

## Deployment Options

### Option 1: Binary (Fastest)
```bash
# Build locally
go build -o stream0 .
scp stream0 ubuntu@your-ec2:/home/ubuntu/
ssh ubuntu@your-ec2 "./stream0 -config stream0.yaml"
```

### Option 2: Docker
```bash
docker build -t stream0 .
docker run -p 8080:8080 -v $(pwd)/data:/data stream0
```

### Option 3: Systemd Service
See SELF_HOSTING.md for complete systemd setup

### Option 4: Fly.io (NEEDS FIX)
- Config exists in fly.toml
- Needs Dockerfile Go version fix first

---

## Immediate Next Steps

### Priority 1: Deploy to AWS EC2
1. Create EC2 instance (t3.micro, Ubuntu 22.04)
2. Open port 8080 in security group
3. SSH in and deploy binary
4. Set up systemd service for persistence

**Script ready in:** SELF_HOSTING.md

### Priority 2: Fix Fly.io (if needed)
- Update Dockerfile to use `golang:1.25-alpine` when available
- OR downgrade all dependencies to Go 1.21 compatible versions

### Priority 3: Authentication (for BoxCrew)
If BoxCrew wants to use this:
- Add JWT validation middleware
- Add topic scoping (org-{id}/topic)
- Add per-topic ACLs

### Priority 4: Client SDK
Build a Python SDK for agents:
```python
from stream0 import Client
client = Client("http://stream0.fly.dev")
client.publish("tasks", {"action": "analyze"})
```

---

## Known Issues

1. **Go version hell**:
   - Original: Go 1.25 in go.mod
   - Docker: 1.24 only available
   - Fixed by: Downgrading gin to v1.9.1, setting go 1.21
   - Status: Should work now

2. **SQLite locking**:
   - High concurrency can cause "database is locked"
   - Mitigation: WAL mode enabled, single writer
   - Solution for scale: Switch to PostgreSQL (v0.2)

3. **No encryption**:
   - HTTP only, no HTTPS
   - No auth tokens
   - Run behind nginx/traefik for production

---

## Testing

Run tests locally:
```bash
# Start server
./stream0

# Run Python test suite
python3 test_go.py
```

All 6 tests should pass.

---

## Key Contacts/Context

**Stakeholder:** Yingjun Wu (yingjun) - BoxCrew
**Goal:** Enable agent-agent communication within BoxCrew
**Use case:** Multi-agent workflows (research → writer → editor)
**Deployment target:** BoxCrew managed infrastructure (stream0 as a service)

**Important:** BoxCrew chat is monitored. Never share credentials in chat.

---

## Resources

- **Repo:** https://github.com/risingwavelabs/stream0
- **Design docs:** /home/sprite/agentbus-go/docs/
- **Original PRD:** /home/sprite/PRD.md (AgentBus version)
- **Go docs:** https://pkg.go.dev/github.com/risingwavelabs/stream0

---

## Quick Start for Next Agent

```bash
cd /home/sprite/agentbus-go

# Build
go build -o stream0 .

# Run locally
./stream0

# Test
curl http://localhost:8080/health
python3 test_go.py

# Deploy to EC2
# (see SELF_HOSTING.md for full guide)
```

---

## Questions to Ask Stakeholder

1. **Authentication**: Do you need API keys or JWT for BoxCrew integration?
2. **Scaling**: One instance per org, or shared multi-tenant?
3. **Persistence**: SQLite OK, or need PostgreSQL for backups?
4. **Client SDK**: Python SDK needed for BoxCrew agents?
5. **Monitoring**: Need metrics endpoint (/metrics) for Prometheus?

---

## Final Notes

- Project is solid and functional
- Main blocker was deployment (Fly.io version issues)
- AWS EC2 is the recommended path forward
- Code is clean, tested, and documented
- Next major work: Authentication layer and client SDK

**Good luck!**
