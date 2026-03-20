#!/usr/bin/env python3
"""
Stream0 Worker — an autonomous agent that polls its inbox and processes tasks.

This script IS the agent. It polls Stream0 for messages, uses Claude to process
each one, and sends the result back. No human in the loop.

Usage:
    export STREAM0_URL=https://stream0.dev
    export STREAM0_API_KEY=sk-xxx
    export STREAM0_AGENT_ID=cao
    export ANTHROPIC_API_KEY=sk-ant-xxx
    python worker.py

The worker will:
1. Register itself on Stream0
2. Poll for unread messages (long-polling, 30s timeout)
3. For each message, invoke Claude to process it
4. Send the result back to the sender via Stream0
5. Ack the message
6. Repeat forever
"""

import os
import sys
import json
import time
import logging
import requests
import anthropic

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    stream=sys.stderr,
)
log = logging.getLogger("stream0-worker")

# Config
STREAM0_URL = os.environ.get("STREAM0_URL", "http://localhost:8080")
STREAM0_API_KEY = os.environ.get("STREAM0_API_KEY", "")
AGENT_ID = os.environ.get("STREAM0_AGENT_ID", "")
ANTHROPIC_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "")

if not AGENT_ID:
    log.error("STREAM0_AGENT_ID not set")
    sys.exit(1)

HEADERS = {"Content-Type": "application/json"}
if STREAM0_API_KEY:
    HEADERS["X-API-Key"] = STREAM0_API_KEY


def stream0_get(path, params=None):
    resp = requests.get(f"{STREAM0_URL}{path}", headers=HEADERS, params=params, timeout=35)
    resp.raise_for_status()
    return resp.json()


def stream0_post(path, data=None):
    resp = requests.post(f"{STREAM0_URL}{path}", headers=HEADERS, json=data, timeout=10)
    resp.raise_for_status()
    return resp.json()


def register():
    stream0_post("/agents", {"id": AGENT_ID})
    log.info(f"Registered as '{AGENT_ID}'")


def check_inbox():
    result = stream0_get(f"/agents/{AGENT_ID}/inbox", {"status": "unread", "timeout": "30"})
    return result.get("messages", [])


def ack(message_id):
    stream0_post(f"/inbox/messages/{message_id}/ack")


def send_reply(to, thread_id, msg_type, content):
    stream0_post(f"/agents/{to}/inbox", {
        "thread_id": thread_id,
        "from": AGENT_ID,
        "type": msg_type,
        "content": content,
    })


def process_with_claude(message):
    """Use Claude to process a message and return the response."""
    client = anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)

    system_prompt = f"""You are {AGENT_ID}, an AI agent. You received a message from another agent through Stream0.
Process the request and return a clear, actionable result. Be concise."""

    user_prompt = f"""Message from: {message.get('from', 'unknown')}
Thread: {message.get('thread_id', 'unknown')}
Type: {message.get('type', 'unknown')}
Content: {json.dumps(message.get('content', {}), indent=2)}

Process this and return the result."""

    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=4096,
        system=system_prompt,
        messages=[{"role": "user", "content": user_prompt}],
    )

    return response.content[0].text


def handle_message(msg):
    """Process a single message: invoke Claude, send result back."""
    msg_id = msg["id"]
    thread_id = msg.get("thread_id", "unknown")
    sender = msg.get("from", "unknown")
    msg_type = msg.get("type", "message")

    log.info(f"Processing [{msg_type}] from {sender} (thread: {thread_id})")

    try:
        # Process with Claude
        result_text = process_with_claude(msg)

        # Determine reply type
        reply_type = "done" if msg_type == "request" else "message"

        # Send result back
        send_reply(sender, thread_id, reply_type, {"result": result_text})
        log.info(f"Replied to {sender} (thread: {thread_id})")

    except Exception as e:
        log.error(f"Failed to process message: {e}")
        try:
            send_reply(sender, thread_id, "failed", {"error": str(e)})
        except Exception:
            pass

    # Always ack
    ack(msg_id)


def main():
    log.info(f"Stream0 Worker starting — agent: {AGENT_ID}, server: {STREAM0_URL}")

    register()

    log.info("Polling for messages... (Ctrl+C to stop)")

    while True:
        try:
            messages = check_inbox()
            for msg in messages:
                handle_message(msg)
        except KeyboardInterrupt:
            log.info("Shutting down")
            break
        except requests.exceptions.Timeout:
            continue  # Normal for long-polling
        except Exception as e:
            log.error(f"Error: {e}")
            time.sleep(5)


if __name__ == "__main__":
    main()
