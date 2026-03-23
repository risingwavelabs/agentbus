# Multi-machine setup

Distribute workers across multiple machines. Each machine uses its own local credentials. No secrets are forwarded.

## Topology

```
                    ┌──────────────────────────┐
                    │      Box0 Server         │
                    │       Machine A          │
                    │  ┌──────────────────────┐│
                    │  │  inbox / routing     ││
                    │  └──────────────────────┘│
                    └────────────┬─────────────┘
                                 │  HTTP
              ┌──────────────────┼──────────────────┐
              │                  │                  │
    ┌─────────▼────────┐ ┌───────▼──────────┐ ┌────▼─────────────┐
    │   Machine A      │ │   Machine B      │ │   Machine C      │
    │   (local node)   │ │  (gpu-box node)  │ │  (cloud node)    │
    │                  │ │                  │ │                  │
    │ ┌──────────────┐ │ │ ┌──────────────┐ │ │ ┌──────────────┐ │
    │ │  ux-expert   │ │ │ │  ml-agent    │ │ │ │  reviewer    │ │
    │ │  architect   │ │ │ │  (GPU tasks) │ │ │ │  (cloud cred)│ │
    │ └──────────────┘ │ │ └──────────────┘ │ │ └──────────────┘ │
    │  own credentials │ │  own credentials │ │  own credentials │
    └──────────────────┘ └──────────────────┘ └──────────────────┘
```

## Setup

### 1. Start the server with external access

The server must bind to `0.0.0.0` for remote machines to connect:

```bash
b0 server --host 0.0.0.0
```

### 2. Join a remote node

On the remote machine, join the server:

```bash
b0 node join http://server-ip:8080 --name gpu-box --key <key>
```

The node daemon starts polling the server for tasks.

### 3. Assign workers to the node

Back on the server machine:

```bash
b0 worker add ml-agent --instructions "ML specialist." --node gpu-box
```

### 4. Delegate tasks

```bash
b0 delegate ml-agent "Analyze this dataset."
```

```bash
b0 wait
```

The task is routed to the remote machine. Claude CLI runs there using that machine's local credentials and compute.

## How it works

- Remote nodes poll the server via HTTP for new tasks.
- Each node runs its own daemon that spawns Claude Code or Codex locally.
- Workers use the machine's existing authentication (OAuth or API key). No credential forwarding.
- Only the node owner can deploy workers to their machine.
- The server handles routing: tasks go to whichever node owns the target worker.

## List nodes

```bash
b0 node ls
```

Shows all connected nodes and their status.
