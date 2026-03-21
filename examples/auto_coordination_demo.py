#!/usr/bin/env python3
"""Runnable Stream0 demo with one primary agent and two background workers."""

from __future__ import annotations

import json
import os
import threading
import time
import urllib.parse
import urllib.request


URL = os.environ.get("STREAM0_URL", "http://localhost:8080")
THREAD_ID = "strategy-memo-1"

PRIMARY_ID = "primary-agent"
RESEARCH_ID = "research-worker"
CRITIC_ID = "critic-worker"


def pretty(data):
    return json.dumps(data, indent=2, ensure_ascii=True)


class Agent:
    def __init__(self, agent_id: str, url: str):
        self.agent_id = agent_id
        self.url = url.rstrip("/")

    def _request(self, method: str, path: str, body=None):
        data = None
        headers = {"Content-Type": "application/json"}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(
            f"{self.url}{path}", data=data, headers=headers, method=method
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def register(self):
        return self._request("POST", "/agents", {"id": self.agent_id})

    def send(self, to: str, thread_id: str, msg_type: str, content=None):
        body = {"thread_id": thread_id, "from": self.agent_id, "type": msg_type}
        if content is not None:
            body["content"] = content
        return self._request("POST", f"/agents/{to}/inbox", body)

    def receive(self, status="unread", thread_id=None, timeout=0):
        params = {}
        if status:
            params["status"] = status
        if thread_id:
            params["thread_id"] = thread_id
        if timeout:
            params["timeout"] = timeout
        query = urllib.parse.urlencode(params)
        suffix = f"?{query}" if query else ""
        result = self._request("GET", f"/agents/{self.agent_id}/inbox{suffix}")
        return result.get("messages", [])

    def ack(self, message_id: str):
        return self._request("POST", f"/inbox/messages/{message_id}/ack")

    def history(self, thread_id: str):
        result = self._request("GET", f"/threads/{thread_id}/messages")
        return result.get("messages", [])


def ack_all(agent: Agent, messages):
    for msg in messages:
        agent.ack(msg["id"])


def run_research_worker(done_event: threading.Event):
    agent = Agent(RESEARCH_ID, url=URL)
    agent.register()

    while not done_event.is_set():
        messages = agent.receive(thread_id=THREAD_ID, timeout=1)
        if not messages:
            continue

        for msg in messages:
            agent.ack(msg["id"])
            if msg["type"] != "request":
                continue

            time.sleep(0.2)
            agent.send(
                PRIMARY_ID,
                thread_id=THREAD_ID,
                msg_type="done",
                content={
                    "worker": RESEARCH_ID,
                    "findings": [
                        "Customers care most about latency, setup time, and auditability.",
                        "Teams already use multiple agents, but coordination is ad hoc.",
                    ],
                },
            )
            done_event.set()


def run_critic_worker(done_event: threading.Event):
    agent = Agent(CRITIC_ID, url=URL)
    agent.register()
    asked_question = False

    while not done_event.is_set():
        messages = agent.receive(thread_id=THREAD_ID, timeout=1)
        if not messages:
            continue

        for msg in messages:
            agent.ack(msg["id"])

            if msg["type"] == "request" and not asked_question:
                asked_question = True
                time.sleep(0.2)
                agent.send(
                    PRIMARY_ID,
                    thread_id=THREAD_ID,
                    msg_type="question",
                    content={
                        "worker": CRITIC_ID,
                        "question": "Should the memo optimize for engineering teams or general knowledge workers?",
                    },
                )
            elif msg["type"] == "answer":
                time.sleep(0.2)
                agent.send(
                    PRIMARY_ID,
                    thread_id=THREAD_ID,
                    msg_type="done",
                    content={
                        "worker": CRITIC_ID,
                        "risks": [
                            "If positioned too broadly, the product sounds like generic chat infrastructure.",
                            "The value only clicks when the primary agent coordinates other agents automatically.",
                        ],
                        "audience": msg["content"]["answer"],
                    },
                )
                done_event.set()


def main():
    primary = Agent(PRIMARY_ID, url=URL)
    primary.register()

    research_done = threading.Event()
    critic_done = threading.Event()

    research_thread = threading.Thread(
        target=run_research_worker, args=(research_done,), daemon=True
    )
    critic_thread = threading.Thread(
        target=run_critic_worker, args=(critic_done,), daemon=True
    )
    research_thread.start()
    critic_thread.start()

    print("================================================")
    print("  Stream0 Demo: Primary Agent Coordinates Others")
    print("================================================")
    print()
    print("User goal:")
    print('  "Write a recommendation memo. Ask other agents to discuss risks and gather market context."')
    print()
    print("What happens next:")
    print("  1. The user talks only to primary-agent.")
    print("  2. primary-agent fans the task out to two specialist workers.")
    print("  3. critic-worker asks a clarification question.")
    print("  4. primary-agent answers and gathers both results.")
    print("  5. primary-agent returns one final result to the user.")
    print()

    user_goal = {
        "goal": "Write a recommendation memo for Stream0.",
        "instructions": [
            "Gather market context from another agent.",
            "Ask another agent to critique the positioning.",
            "Return one final recommendation to the user.",
        ],
    }

    print("--- User -> primary-agent ---")
    print(pretty(user_goal))
    print()

    primary.send(
        RESEARCH_ID,
        thread_id=THREAD_ID,
        msg_type="request",
        content={
            "task": "Gather market context for Stream0.",
            "deliverable": "2 short findings for the memo.",
        },
    )
    primary.send(
        CRITIC_ID,
        thread_id=THREAD_ID,
        msg_type="request",
        content={
            "task": "Critique the positioning of Stream0.",
            "deliverable": "Main risks or objections.",
        },
    )

    print("--- primary-agent fan-out ---")
    print(f"Sent request to {RESEARCH_ID}")
    print(f"Sent request to {CRITIC_ID}")
    print()

    findings = []
    risks = []
    audience = None
    received_workers = set()

    while received_workers != {RESEARCH_ID, CRITIC_ID}:
        messages = primary.receive(thread_id=THREAD_ID, timeout=5)
        if not messages:
            continue

        ack_all(primary, messages)

        for msg in messages:
            print(f"--- {msg['from']} -> primary-agent ({msg['type']}) ---")
            print(pretty(msg["content"]))
            print()

            if msg["type"] == "question":
                answer = "Optimize for engineering teams first."
                primary.send(
                    CRITIC_ID,
                    thread_id=THREAD_ID,
                    msg_type="answer",
                    content={"answer": answer},
                )
                print("--- primary-agent -> critic-worker (answer) ---")
                print(pretty({"answer": answer}))
                print()
            elif msg["type"] == "done":
                worker = msg["content"]["worker"]
                received_workers.add(worker)
                if worker == RESEARCH_ID:
                    findings = msg["content"]["findings"]
                elif worker == CRITIC_ID:
                    risks = msg["content"]["risks"]
                    audience = msg["content"]["audience"]

    final_result = {
        "summary": "Position Stream0 as the messaging layer behind one primary agent that coordinates other agents.",
        "target_audience": audience,
        "market_context": findings,
        "risks": risks,
    }

    print("--- primary-agent -> user ---")
    print(pretty(final_result))
    print()

    time.sleep(0.5)
    history = primary.history(THREAD_ID)
    print("--- Thread history ---")
    print(pretty({"thread_id": THREAD_ID, "messages": history}))
    print()
    print("Demo complete.")


if __name__ == "__main__":
    main()
