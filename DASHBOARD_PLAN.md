# Dashboard Redesign Plan

## Current State

- Login page with API key input
- Sidebar: Workers, Tasks, Nodes, Team (4 pages)
- Default route: `#/workers` (worker list table)
- Worker detail page: info + last 20 raw messages
- Tasks page: pending inbox messages grouped by thread (N+1 queries)
- Nodes page: node cards with workers listed
- Team page: groups + users management

## Problems

- No overview page. Login lands on a flat worker table.
- "Tasks" is a manufactured concept. Users think in terms of workers and conversations, not inbox messages.
- Nodes and Workers are separate pages, but users think "what's running where."
- "Team" is not a real concept in the backend. Backend has groups and users.
- No way to talk to a worker from the UI. Must use CLI.
- No way to create a temp worker (quick one-off task) from the UI.

## New Design

### Sidebar

```
Dashboard      <- default, nodes + workers overview
Groups         <- group list, create group, add members
Users          <- user list, invite (admin only)

[Group: my-team v]   <- group selector, bottom of sidebar
[username]
[Sign out]
```

Three navigation items. No invented concepts. Group selector stays at the bottom.

### Route Map

```
#/dashboard                       <- Dashboard (default after login)
#/workers/detail/{name}           <- Worker detail + threads + conversation
#/workers/detail/{name}/{thread}  <- Worker detail with thread expanded
#/groups                          <- Groups management
#/users                           <- Users management (admin only)
```

### Dashboard Page

Fetch: `GET /nodes` + `GET /groups/{group}/workers`

Layout: each node is a card. Workers grouped by `node_id` inside each card.

```
+---------------------------------------------------+
|  Dashboard                        [+ Quick Task]  |
+---------------------------------------------------+

+---------------------------------------------------+
| local                                   * online  |
|                                                   |
| +------------+ +------------+ +------------+      |
| | reviewer   | | coder      | | tester     |      |
| | * active   | | * active   | | * stopped  |      |
| | 3 threads  | | 1 thread   | | 0 threads  |      |
| +------------+ +------------+ +------------+      |
+---------------------------------------------------+

+---------------------------------------------------+
| gpu-server                              * online  |
|                                                   |
| +------------+                                    |
| | trainer    |                                    |
| | * active   |                                    |
| | 0 threads  |                                    |
| +------------+                                    |
+---------------------------------------------------+
```

Each worker block is clickable. Goes to `#/workers/detail/{name}`.

Thread count comes from: `GET /groups/{group}/workers/{name}/threads` (new endpoint, already implemented).

If a worker has unread questions or recent failures, show a visual indicator (colored border or badge).

"+ Quick Task" button opens a modal (see below).

### Worker Detail Page

Fetch: `GET /groups/{group}/workers/{name}` + `GET /groups/{group}/workers/{name}/threads`

```
+---------------------------------------------------+
|  < Dashboard / reviewer        [Stop] [Remove]    |
+---------------------------------------------------+

+---------------------------------------------------+
| Details                                           |
|                                                   |
| Name:          reviewer                           |
| Description:   Reviews PRs for security issues    |
| Node:          local                              |
| Runtime:       claude                             |
| Status:        * active                           |
| Instructions:  "Review code carefully..."         |
+---------------------------------------------------+

+---------------------------------------------------+
| Conversations                [+ New Conversation] |
|                                                   |
| thread-a8f3  "Review PR #123"     * done     2m  |
| thread-b2c1  "Review PR #456"     * question 1h  |
| thread-c9d4  "Fix auth bug"       * failed   3h  |
+---------------------------------------------------+
```

Click a thread row to expand it inline:

```
| v thread-b2c1  "Review PR #456"   * question 1h  |
| +-----------------------------------------------+ |
| | You: Review PR #456 for security issues       | |
| |                                               | |
| | reviewer: I found 2 issues. Should I also     | |
| | check the test coverage?                      | |
| |                                               | |
| | [____________________________] [Send]         | |
| +-----------------------------------------------+ |
```

### Conversation Mechanics

**New conversation:**
1. Click "+ New Conversation"
2. Modal with textarea for task content
3. On submit:
   - Register agent: `POST /groups/{group}/agents` with id = `web-{uuid}`
   - Generate thread_id = `thread-{uuid}`
   - Send message: `POST /groups/{group}/agents/{worker}/inbox` with `{thread_id, from: "web-{uuid}", type: "request", content: "..."}`
   - Store `web-{uuid}` in localStorage as the user's web agent identity (reuse across sessions)
4. Thread appears in list with "working..." state
5. Frontend polls `GET /groups/{group}/threads/{thread_id}` every 3 seconds
6. When a new message appears (done/question/failed), stop polling, show result

**Reply to question:**
1. In expanded thread, type reply in input box
2. On submit: `POST /groups/{group}/agents/{worker}/inbox` with `{thread_id, from: "web-{uuid}", type: "answer", content: "..."}`
3. Thread goes back to "working..." state
4. Resume polling until next response

**Polling behavior:**
- Only poll threads that are in "working" state (last message was request or answer from user)
- Poll interval: 3 seconds
- Stop polling when thread gets a done/question/failed response
- Show elapsed time: "reviewer is working... (12s)"

### Quick Task (Temp Worker)

Click "+ Quick Task" on Dashboard. Modal:

```
+-------------------------------------------+
| Quick Task                                |
|                                           |
| Instructions: [________________________] |
| Task:         [________________________] |
| Node:         [local v]                  |
| Runtime:      [auto v]                   |
|                                           |
|                     [Cancel] [Run]        |
+-------------------------------------------+
```

On submit:
1. Create worker: `POST /groups/{group}/workers` with auto-generated name like `task-{short-uuid}`
2. Send task to it (same as new conversation flow)
3. Redirect to `#/workers/detail/{name}` to watch progress

Cleanup: for now, user manually removes temp workers. Automatic cleanup can be added later on the backend.

### Groups Page

Same as current "Team" page groups section, renamed.

Fetch: `GET /groups`

```
+---------------------------------------------------+
|  Groups                         [+ Create Group]  |
+---------------------------------------------------+

+---------------------------------------------------+
| Name        | Created By | Created   |            |
|-------------|------------|-----------|-------------|
| my-team     | u-abc123   | 2d ago    | [Add Member]|
| personal    | u-abc123   | 5d ago    | [Add Member]|
+---------------------------------------------------+
```

### Users Page

Same as current "Team" page users section.

Fetch: `GET /users` (admin only, returns 403 for non-admin)

Non-admin: show "User management is only available to admins."

Admin:

```
+---------------------------------------------------+
|  Users                          [+ Invite User]   |
+---------------------------------------------------+

+---------------------------------------------------+
| ID         | Name  | Admin | Created              |
|------------|-------|-------|----------------------|
| u-abc123   | admin | Yes   | 5d ago               |
| u-def456   | alice | No    | 2d ago               |
+---------------------------------------------------+
```

## Backend Changes (Already Done)

- Added `ThreadSummary` model in db.rs
- Added `get_worker_threads()` query in db.rs
- Added `GET /groups/{group}/workers/{name}/threads` route in server.rs
- Returns: `{threads: [{thread_id, first_content, latest_type, latest_at, message_count}]}`

## Backend Changes (Not Yet Needed)

No additional backend changes required for this plan. All operations use existing API endpoints:

- `GET /nodes` - list nodes
- `GET /groups/{group}/workers` - list workers
- `GET /groups/{group}/workers/{name}` - worker detail
- `GET /groups/{group}/workers/{name}/threads` - thread list (new)
- `GET /groups/{group}/threads/{thread_id}` - thread messages
- `POST /groups/{group}/agents` - register web agent
- `POST /groups/{group}/agents/{worker}/inbox` - send message
- `POST /groups/{group}/workers` - create worker (for quick task)
- `DELETE /groups/{group}/workers/{name}` - remove worker
- `POST /groups/{group}/workers/{name}/stop` - stop worker
- `POST /groups/{group}/workers/{name}/start` - start worker
- `GET /groups` - list groups
- `POST /groups` - create group
- `POST /groups/{group}/members/{user_id}` - add member
- `GET /users` - list users (admin)
- `POST /users/invite` - invite user (admin)

## Frontend Implementation

Single file: `web/index.html`. Vanilla JS, no framework, no build step.

Rewrite the existing file. Keep the same patterns (App namespace, hash routing, h() helper, toast notifications, modal pattern). Replace all page modules with the new structure.

## What Gets Deleted

- `App.tasks` module (entire Tasks page)
- `App.nodes` module (entire Nodes page)
- `App.team` module (replaced by App.groups + App.users)
- Sidebar nav items for Workers, Tasks, Nodes, Team
- Route handlers for `/workers`, `/tasks`, `/nodes`, `/team`
