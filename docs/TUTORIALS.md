# stream0 Tutorials

Hands-on demos for the four core messaging patterns. All examples use `curl` — any language with HTTP support works the same way.

## Setup

```bash
# Set these for all examples
export BASE="http://<your-stream0-host>:8080"
export API_KEY="<your-api-key>"
```

---

## Demo 1: Multi-Agent Pipeline

**Pattern:** Sequential processing across stages — each agent consumes from one topic and produces to the next.

```
User Request → [research topic] → Research Agent → [drafts topic] → Writer Agent → [published topic] → Editor Agent
```

```bash
# Create pipeline topics
curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"pipeline-research","retention_days":1}'

curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"pipeline-drafts","retention_days":1}'

curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"pipeline-published","retention_days":1}'

# User submits a request
curl -s -X POST $BASE/topics/pipeline-research/messages \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"payload":{"task":"Write a blog post about SQLite","requested_by":"user","priority":"high"}}'

# Research Agent: consume task, produce findings
RESEARCH=$(curl -s "$BASE/topics/pipeline-research/messages?group=researchers&max=1&timeout=5" \
  -H "X-API-Key: $API_KEY")
echo "$RESEARCH" | python3 -c "import sys,json; m=json.load(sys.stdin)['messages'][0]; print(f'Research task: {m[\"payload\"][\"task\"]}')"

# Extract message ID and ack
MSG_ID=$(echo "$RESEARCH" | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['id'])")

# Publish research findings to drafts
curl -s -X POST $BASE/topics/pipeline-drafts/messages \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"payload":{"title":"Why SQLite is Great","findings":["Zero config","ACID compliant","WAL mode"]}}'

# Ack the research task
curl -s -X POST $BASE/messages/$MSG_ID/ack \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"group":"researchers"}'

# Writer Agent: consume research, produce article
DRAFT=$(curl -s "$BASE/topics/pipeline-drafts/messages?group=writers&max=1&timeout=5" \
  -H "X-API-Key: $API_KEY")
DRAFT_ID=$(echo "$DRAFT" | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['id'])")

curl -s -X POST $BASE/topics/pipeline-published/messages \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"payload":{"title":"Why SQLite is Great","body":"Full article text here...","status":"needs_review"}}'

curl -s -X POST $BASE/messages/$DRAFT_ID/ack \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"group":"writers"}'

# Editor Agent: consume and approve
ARTICLE=$(curl -s "$BASE/topics/pipeline-published/messages?group=editors&max=1&timeout=5" \
  -H "X-API-Key: $API_KEY")
ART_ID=$(echo "$ARTICLE" | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['id'])")

curl -s -X POST $BASE/messages/$ART_ID/ack \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"group":"editors"}'

echo "Pipeline complete!"
```

**When to use:** Content creation, data processing pipelines, multi-step workflows where each stage is a different agent.

---

## Demo 2: Fan-Out Broadcast

**Pattern:** One event is received by multiple independent subscribers. Each consumer group gets every message.

```
Deployment Event → [system-events topic] → Logger (group: loggers)
                                         → Metrics (group: metrics)
                                         → Notifier (group: notifiers)
```

```bash
# Create events topic
curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"system-events","retention_days":1}'

# Publish one event
curl -s -X POST $BASE/topics/system-events/messages \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"payload":{"event":"deploy","service":"stream0","version":"0.2.0"}}'

# Three different groups each receive the same event
curl -s "$BASE/topics/system-events/messages?group=loggers&max=1&timeout=2" -H "X-API-Key: $API_KEY"
curl -s "$BASE/topics/system-events/messages?group=metrics&max=1&timeout=2" -H "X-API-Key: $API_KEY"
curl -s "$BASE/topics/system-events/messages?group=notifiers&max=1&timeout=2" -H "X-API-Key: $API_KEY"

# All three get the same message!
```

**When to use:** Event notifications, audit logging, syncing multiple systems from one event source.

---

## Demo 3: Competing Consumers (Load Balancing)

**Pattern:** Multiple workers in the same consumer group split work — each message goes to exactly one worker.

```
10 tasks → [work-queue topic] → Worker A (group: pool) gets some
                               → Worker B (group: pool) gets the rest
                               → Worker C (group: pool) gets none (all claimed)
```

```bash
# Create work queue
curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"work-queue","retention_days":1}'

# Publish 10 tasks
for i in $(seq 1 10); do
  curl -s -X POST $BASE/topics/work-queue/messages \
    -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
    -d "{\"payload\":{\"task_id\":$i,\"action\":\"process-image-$i\"}}"
done

# Workers in the SAME group compete for tasks
# Worker A
curl -s "$BASE/topics/work-queue/messages?group=pool&max=10&timeout=2" -H "X-API-Key: $API_KEY"

# Worker B gets whatever A didn't claim
curl -s "$BASE/topics/work-queue/messages?group=pool&max=10&timeout=2" -H "X-API-Key: $API_KEY"

# No duplicates — the same message is never delivered to two workers in the same group
```

**When to use:** Task queues, background job processing, distributing work across a pool of agents.

---

## Demo 4: Visibility Timeout & Automatic Retry

**Pattern:** If a worker crashes (doesn't ack), the message becomes available again after the visibility timeout expires.

```
Job published → Worker 1 claims (5s timeout) → Worker 1 crashes (no ack)
             → 5 seconds pass...
             → Worker 2 claims same job → Worker 2 processes and acks ✓
```

```bash
# Create topic
curl -s -X POST $BASE/topics -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"name":"critical-jobs","retention_days":1}'

# Publish a critical job
curl -s -X POST $BASE/topics/critical-jobs/messages \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"payload":{"job":"send-invoice","customer":"acme-corp","amount":5000}}'

# Worker 1 claims with SHORT 5-second visibility timeout
W1=$(curl -s "$BASE/topics/critical-jobs/messages?group=billing&max=1&timeout=2&visibility_timeout=5" \
  -H "X-API-Key: $API_KEY")
echo "Worker 1 claimed: $(echo $W1 | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['id'])")"
echo "Worker 1 crashes! (no ack)"

# Wait for timeout to expire
sleep 6

# Worker 2 picks up the SAME message (auto-redelivered)
W2=$(curl -s "$BASE/topics/critical-jobs/messages?group=billing&max=1&timeout=2" \
  -H "X-API-Key: $API_KEY")
W2_ID=$(echo $W2 | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['id'])")
W2_DC=$(echo $W2 | python3 -c "import sys,json; print(json.load(sys.stdin)['messages'][0]['delivery_count'])")
echo "Worker 2 claimed: $W2_ID (delivery_count: $W2_DC)"

# Worker 2 acks
curl -s -X POST $BASE/messages/$W2_ID/ack \
  -H "X-API-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"group":"billing"}'

echo "No data loss despite worker crash!"
```

**When to use:** Any critical task where you can't afford to lose messages — billing, notifications, data processing.

---

## Summary

| Pattern | Same Group? | Behavior |
|---------|------------|----------|
| **Pipeline** | Different groups per stage | Each stage processes sequentially |
| **Broadcast** | Different groups | Every group gets every message |
| **Load Balance** | Same group | Each message goes to one worker |
| **Retry** | Same group + timeout | Unacked messages redeliver automatically |

## Key Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `group` | (required) | Consumer group name |
| `max` | 10 | Max messages to return (1-100) |
| `timeout` | 5 | Long-poll timeout in seconds (0-30) |
| `visibility_timeout` | 30 | Seconds before unacked message is redelivered (5-300) |
