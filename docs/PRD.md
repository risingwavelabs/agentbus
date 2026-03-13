# AgentBus - Product Requirements Document

## 1. Overview

AgentBus is a lightweight, HTTP-native event streaming system designed specifically for AI agent coordination. It bridges the gap between simple message queues and complex streaming platforms, providing exactly what agents need without the operational overhead of Kafka or the infrastructure requirements of Redis.

## 2. Problem Statement

### 2.1 Current Landscape Issues

| System | Problems for Agents |
|--------|---------------------|
| **Apache Kafka** | Complex protocol, requires client libraries, partition management overhead, heavy infrastructure |
| **Redis Streams** | Requires Redis server, not durable by default, limited consumer group semantics |
| **NATS** | Requires server deployment, not zero-setup, learning curve |
| **AWS SQS/SNS** | Vendor lock-in, complex IAM, not developer-friendly for local testing |
| **RabbitMQ** | Heavy broker, AMQP complexity, exchange/binding overhead |

### 2.2 Agent-Specific Needs

Agents are different from traditional services:
- **Ephemeral execution**: Spawn, process, die (seconds to minutes)
- **Polyglot runtime**: Python, Node, Go, shell scripts - anything with HTTP
- **Autonomous coordination**: No central orchestrator, self-organizing via events
- **Debugging needs**: Must replay message history to understand decisions

## 3. Goals

### 3.1 Primary Goals

**Zero Setup**: Single binary, runs locally with no dependencies
**HTTP Native**: REST API + WebSocket, no special client libraries
**Agent Semantics**: Consumer groups, exactly-once processing, dead letter queues
**Observable**: Built-in message inspection and replay

### 3.2 Non-Goals

Multi-datacenter replication
Million messages/second throughput
Kafka protocol compatibility
Complex stream processing (joins, windows)

## 4. Use Cases

### 4.1 Multi-Agent Workflow
```
User: "Plan my Tokyo trip"

Research Agent → publishes to "research.done"
    ↓
Writer Agent ← consumes, writes draft → publishes to "draft.ready"
    ↓
Editor Agent ← consumes, reviews
```

### 4.2 Human-in-the-Loop

AI Agent → publishes to "approvals.pending"
    ↓
Human reviews in UI
    ↓
publishes to "approvals.approved"
    ↓
AI Agent ← resumes workflow

### 4.3 Load Balancing
```
1000 documents to process

5 Document Processor Agent instances
    ↓ all subscribe to "documents.new"
Each claims one, processes, commits
Work distributes automatically
```

### 4.4 Real-Time Collaboration

Agent A typing summary
    ↓ publishes streaming tokens
Agent B watching "summaries.updates"
    ↓ provides live feedback

### 4.5 Debugging & Replay
```
Agent made wrong decision yesterday

Replay: messages from 2024-03-12 14:00 to 14:05
See exactly what it saw, reproduce the bug
```

## 5. Architecture

### 5.1 Deployment Modes

#### Mode 1: Embedded (Development)

Agent process
├── Agent logic
└── AgentBus (SQLite + HTTP on localhost:8080)

- Single binary, zero network calls
- Isolated per agent (no sharing)

#### Mode 2: Sidecar (Team Development)

Docker Container / VM
├── Agent process A
├── Agent process B
└── AgentBus process (shared via localhost:8080)

- Multiple agents share state
- Single compose file startup

#### Mode 3: Centralized (Production)

Agent A ──┐
Agent B ──┼──→ https://agentbus.company.com
Agent C ──┘

- Shared across machines
- Persistent, HA deployment

### 5.2 Storage Architecture

**SQLite Backend**
- Single-file database
- ACID transactions
- Handles 10K+ writes/sec (sufficient for agent workloads)
- Easy backup/restore

**Storage Schema**
```sql
-- Topics table
topics (id, name, created_at, retention_days)

-- Messages table
messages (id, topic_id, payload, headers, timestamp, offset)

-- Consumer groups
consumer_groups (id, name, topic_id, created_at)

-- Consumer offsets
offsets (group_id, topic_id, partition, offset, updated_at)

-- Dead letter queue
dlq (id, message_id, topic_id, error, retries, created_at)
```

## 6. API Specification

### 6.1 Produce Messages

POST /topics/{topic}/messages
Content-Type: application/json

{
  "payload": { "any": "json" },
  "headers": { "trace-id": "abc123" },  // optional
  "key": "user-123"  // optional, for partitioning
}

**Response:**
```json
{
  "message_id": "msg-uuid",
  "offset": 42,
  "timestamp": "2024-03-12T14:30:00Z"
}
```

### 6.2 Consume Messages (Polling)

GET /topics/{topic}/messages?group={group}&max={n}&timeout={seconds}

**Response:**
```json
{
  "messages": [
    {
      "id": "msg-uuid",
      "offset": 42,
      "payload": { ... },
      "headers": { ... },
      "timestamp": "2024-03-12T14:30:00Z",
      "delivery_count": 1
    }
  ]
}
```

### 6.3 Acknowledge Messages

POST /messages/{message_id}/ack

{
  "group": "my-consumer-group"
}

**Automatic ack options:**
- Immediate: Ack on HTTP 200
- Explicit: Require separate ack call
- Timeout: Auto-ack after N seconds if not rejected

### 6.4 WebSocket Streaming

WS /topics/{topic}/subscribe?group={group}

**Inbound:** Server pushes messages as JSON
**Outbound:** Client sends acks, nacks, ping

### 6.5 List Topics

GET /topics

### 6.6 Create Topic

POST /topics

{
  "name": "tasks.new",
  "partitions": 1,  // optional, default 1
  "retention_days": 7  // optional
}

### 6.7 Message Replay

GET /topics/{topic}/messages?start={timestamp}&end={timestamp}

### 6.8 Dead Letter Queue

GET /topics/{topic}/dlq
POST /messages/{id}/retry  // Move from DLQ back to topic

## 7. Consumer Groups

### 7.1 Behavior

Multiple consumers in same group = load balancing
Each message delivered to exactly one consumer in group
Consumer failure → message redelivered to another
Auto-rebalance when consumers join/leave

### 7.2 Configuration

{
  "group": "my-workers",
  "auto_ack": false,
  "max_delivery_attempts": 3,
  "visibility_timeout_seconds": 30
}

## 8. Data Volume & Performance

### 8.1 Expected Scale

| Metric | Target | Notes |
|--------|--------|-------|
| Throughput | 10K msg/sec | Per instance, sufficient for agent workloads |
| Message size | 10KB average | Text-heavy agent outputs |
| Storage | 100GB per topic | Configurable retention |
| Latency p99 | <100ms | Local/edge deployment |

### 8.2 Resource Requirements

**Minimum:**
- 256MB RAM
- 1 CPU core
- 1GB disk

**Recommended:**
- 1GB RAM
- 2 CPU cores
- SSD storage

## 9. Testing Strategy

### 9.1 Unit Tests

Message ordering within partition
Consumer group assignment logic
At-least-once delivery guarantee
Offset management correctness

### 9.2 Integration Tests

```python
# Scenario: Basic pub/sub
producer.send(topic="orders", data={"item": "laptop"})
msg = consumer.poll(topic="orders", group="processors")
assert msg.data["item"] == "laptop"

# Scenario: Consumer group load balancing
# Start 3 consumers, send 10 messages
# Verify each gets ~3-4, total = 10, no duplicates

# Scenario: Failure recovery
consumer1.poll()  # Gets message, crashes before ack
consumer2.poll()  # Same message redelivered
```

### 9.3 Chaos Tests

Kill broker mid-publish, verify durability
Network partition, verify queue behavior
Consumer crash, verify redelivery

## 10. Security

### 10.1 Authentication (v2)

API key-based auth
JWT tokens for short-lived agents

### 10.2 Authorization (v2)

Topic-level ACLs
Produce/consume permissions per API key

### 10.3 Encryption

TLS for HTTP/WebSocket
At-rest encryption via filesystem

## 11. Observability

### 11.1 Metrics

Messages produced/consumed per topic
Consumer lag per group
DLQ depth
Latency histograms

### 11.2 Tracing

Correlation IDs via headers
End-to-end message flow visualization

## 12. Roadmap

### MVP (v0.1)
- [ ] HTTP produce/consume
- [ ] Consumer groups
- [ ] SQLite persistence
- [ ] WebSocket streaming

### v0.2
- [ ] Dead letter queue
- [ ] Message replay
- [ ] Docker image
- [ ] Basic auth

### v0.3
- [ ] Clustering (3-node HA)
- [ ] Metrics endpoint
- [ ] Admin UI
- [ ] Cloud offering

## 13. Open Questions

Should we support message priorities?
What's the right default retention? (7 days? 30 days?)
Do agents need scheduled messages ("deliver at time T")?
Should we support message TTLs?
Push-based delivery vs only pull?

## 14. Success Metrics

Time to first message: <30 seconds (download → produce)
Lines of code to integrate: <10
Local testing: Works without Docker
Production migration: Just change URL
