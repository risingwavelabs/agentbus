use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
}

// --- Models ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxMessage {
    pub id: String,
    pub thread_id: String,
    #[serde(rename = "from")]
    pub from_agent: String,
    #[serde(rename = "to")]
    pub to_agent: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub content: Option<serde_json::Value>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub name: String,
    pub instructions: String,
    pub node_id: String,
    pub status: String,
    pub registered_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub owner: String,
    pub status: String,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn =
            Connection::open(path).with_context(|| format!("failed to open database: {}", path))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id TEXT NOT NULL PRIMARY KEY,
                name TEXT NOT NULL,
                key TEXT NOT NULL UNIQUE,
                is_admin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS groups (
                name TEXT NOT NULL PRIMARY KEY,
                created_by TEXT NOT NULL,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS group_members (
                group_name TEXT NOT NULL,
                user_id TEXT NOT NULL,
                PRIMARY KEY (group_name, user_id),
                FOREIGN KEY (group_name) REFERENCES groups(name),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );

            CREATE TABLE IF NOT EXISTS agents (
                group_name TEXT NOT NULL,
                id TEXT NOT NULL,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                last_seen TEXT,
                PRIMARY KEY (group_name, id)
            );

            CREATE TABLE IF NOT EXISTS inbox_messages (
                id TEXT PRIMARY KEY,
                group_name TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                type TEXT NOT NULL,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'unread',
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT NOT NULL PRIMARY KEY,
                owner TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'online',
                last_heartbeat TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                FOREIGN KEY (owner) REFERENCES users(id)
            );

            CREATE TABLE IF NOT EXISTS workers (
                group_name TEXT NOT NULL,
                name TEXT NOT NULL,
                instructions TEXT NOT NULL,
                node_id TEXT NOT NULL DEFAULT 'local',
                status TEXT NOT NULL DEFAULT 'active',
                registered_by TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (group_name, name)
            );

            CREATE INDEX IF NOT EXISTS idx_inbox_group_to_status ON inbox_messages(group_name, to_agent, status);
            CREATE INDEX IF NOT EXISTS idx_inbox_group_thread ON inbox_messages(group_name, thread_id);
            ",
        )?;
        Ok(())
    }

    fn parse_ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
                    .map(|ndt| ndt.and_utc())
            })
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                    .map(|ndt| ndt.and_utc())
            })
            .unwrap_or_else(|_| Utc::now())
    }

    // --- Users ---

    pub fn create_user(&self, name: &str, is_admin: bool) -> Result<(User, String)> {
        let conn = self.conn.lock().unwrap();
        let id = format!("u-{}", &Uuid::new_v4().to_string()[..8]);
        let key = format!("b0_{}", &Uuid::new_v4().to_string().replace('-', ""));

        conn.execute(
            "INSERT INTO users (id, name, key, is_admin) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, key, is_admin as i32],
        )?;

        // Auto-create personal group
        conn.execute(
            "INSERT INTO groups (name, created_by) VALUES (?1, ?2)",
            params![name, id],
        )?;
        conn.execute(
            "INSERT INTO group_members (group_name, user_id) VALUES (?1, ?2)",
            params![name, id],
        )?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM users WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        Ok((
            User {
                id,
                name: name.to_string(),
                is_admin,
                created_at: Self::parse_ts(&ts),
            },
            key,
        ))
    }

    /// Validate a key. Returns the User if valid.
    pub fn authenticate(&self, key: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, is_admin, created_at FROM users WHERE key = ?1",
            params![key],
            |row| {
                let ts: String = row.get(3)?;
                Ok(User {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_admin: row.get::<_, i32>(2)? != 0,
                    created_at: Database::parse_ts(&ts),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_admin_user_id(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM users WHERE is_admin = 1 LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, is_admin, created_at FROM users ORDER BY created_at ASC",
        )?;
        let users = stmt
            .query_map([], |row| {
                let ts: String = row.get(3)?;
                Ok(User {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_admin: row.get::<_, i32>(2)? != 0,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(users)
    }

    /// Bootstrap: create admin user if none exists. Returns (user, key) if created.
    pub fn bootstrap_admin(&self) -> Result<Option<(User, String)>> {
        let conn = self.conn.lock().unwrap();
        let has_admin: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE is_admin = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)?;

        if has_admin {
            return Ok(None);
        }
        drop(conn);
        Ok(Some(self.create_user("admin", true)?))
    }

    // --- Groups ---

    pub fn create_group(&self, name: &str, created_by: &str) -> Result<Group> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO groups (name, created_by) VALUES (?1, ?2)",
            params![name, created_by],
        )?;
        // Creator is automatically a member
        conn.execute(
            "INSERT INTO group_members (group_name, user_id) VALUES (?1, ?2)",
            params![name, created_by],
        )?;
        let ts: String = conn.query_row(
            "SELECT created_at FROM groups WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(Group {
            name: name.to_string(),
            created_by: created_by.to_string(),
            created_at: Self::parse_ts(&ts),
        })
    }

    pub fn add_group_member(&self, group_name: &str, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO group_members (group_name, user_id) VALUES (?1, ?2)",
            params![group_name, user_id],
        )?;
        Ok(())
    }

    pub fn list_groups_for_user(&self, user_id: &str) -> Result<Vec<Group>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT g.name, g.created_by, g.created_at FROM groups g
             JOIN group_members gm ON g.name = gm.group_name
             WHERE gm.user_id = ?1 ORDER BY g.name ASC",
        )?;
        let groups = stmt
            .query_map(params![user_id], |row| {
                let ts: String = row.get(2)?;
                Ok(Group {
                    name: row.get(0)?,
                    created_by: row.get(1)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(groups)
    }

    pub fn is_group_member(&self, group_name: &str, user_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM group_members WHERE group_name = ?1 AND user_id = ?2",
            params![group_name, user_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .map_err(Into::into)
    }

    // --- Agents (inbox identities) ---

    pub fn register_agent(&self, group_name: &str, id: &str) -> Result<Agent> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO agents (group_name, id) VALUES (?1, ?2)",
            params![group_name, id],
        )?;
        let ts: String = conn.query_row(
            "SELECT created_at FROM agents WHERE group_name = ?1 AND id = ?2",
            params![group_name, id],
            |row| row.get(0),
        )?;
        Ok(Agent {
            id: id.to_string(),
            created_at: Database::parse_ts(&ts),
            last_seen: None,
        })
    }

    pub fn resolve_agent(&self, group_name: &str, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE group_name = ?1 AND id = ?2",
                params![group_name, id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)?;
        if exists {
            Ok(Some(id.to_string()))
        } else {
            Ok(None)
        }
    }

    pub fn update_last_seen(&self, group_name: &str, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET last_seen = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE group_name = ?1 AND id = ?2",
            params![group_name, agent_id],
        )?;
        Ok(())
    }

    // --- Inbox Messages ---

    pub fn send_inbox_message(
        &self,
        group_name: &str,
        thread_id: &str,
        from: &str,
        to: &str,
        msg_type: &str,
        content: Option<&serde_json::Value>,
    ) -> Result<InboxMessage> {
        let conn = self.conn.lock().unwrap();
        let msg_id = format!("imsg-{}", &Uuid::new_v4().to_string()[..16]);
        let content_str = content.map(|c| serde_json::to_string(c).unwrap_or_default());

        conn.execute(
            "INSERT INTO inbox_messages (id, group_name, thread_id, from_agent, to_agent, type, content) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg_id, group_name, thread_id, from, to, msg_type, content_str],
        )?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM inbox_messages WHERE id = ?1",
            params![msg_id],
            |row| row.get(0),
        )?;

        Ok(InboxMessage {
            id: msg_id,
            thread_id: thread_id.to_string(),
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            msg_type: msg_type.to_string(),
            content: content.cloned(),
            status: "unread".to_string(),
            created_at: Self::parse_ts(&ts),
        })
    }

    pub fn get_inbox_messages(
        &self,
        group_name: &str,
        agent_id: &str,
        status: Option<&str>,
        thread_id: Option<&str>,
    ) -> Result<Vec<InboxMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut query =
            "SELECT id, thread_id, from_agent, to_agent, type, content, status, created_at FROM inbox_messages WHERE group_name = ?1 AND to_agent = ?2"
                .to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(group_name.to_string()), Box::new(agent_id.to_string())];
        let mut param_idx = 3;

        if let Some(s) = status {
            query += &format!(" AND status = ?{}", param_idx);
            param_values.push(Box::new(s.to_string()));
            param_idx += 1;
        }
        if let Some(t) = thread_id {
            query += &format!(" AND thread_id = ?{}", param_idx);
            param_values.push(Box::new(t.to_string()));
        }
        query += " ORDER BY created_at ASC";

        let mut stmt = conn.prepare(&query)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let messages = stmt
            .query_map(params_ref.as_slice(), |row| {
                let content_str: Option<String> = row.get(5)?;
                let ts: String = row.get(7)?;
                Ok(InboxMessage {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    from_agent: row.get(2)?,
                    to_agent: row.get(3)?,
                    msg_type: row.get(4)?,
                    content: content_str.and_then(|s| serde_json::from_str(&s).ok()),
                    status: row.get(6)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn ack_inbox_message(&self, group_name: &str, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE inbox_messages SET status = 'acked' WHERE id = ?1 AND group_name = ?2 AND status = 'unread'",
            params![message_id, group_name],
        )?;
        if updated == 0 {
            anyhow::bail!("message not found or already acked");
        }
        Ok(())
    }

    pub fn get_thread_messages(
        &self,
        group_name: &str,
        thread_id: &str,
    ) -> Result<Vec<InboxMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, from_agent, to_agent, type, content, status, created_at
             FROM inbox_messages WHERE group_name = ?1 AND thread_id = ?2 ORDER BY created_at ASC",
        )?;
        let messages = stmt
            .query_map(params![group_name, thread_id], |row| {
                let content_str: Option<String> = row.get(5)?;
                let ts: String = row.get(7)?;
                Ok(InboxMessage {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    from_agent: row.get(2)?,
                    to_agent: row.get(3)?,
                    msg_type: row.get(4)?,
                    content: content_str.and_then(|s| serde_json::from_str(&s).ok()),
                    status: row.get(6)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    // --- Nodes ---

    pub fn register_node(&self, id: &str, owner: &str) -> Result<Node> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO nodes (id, owner, status, last_heartbeat) VALUES (?1, ?2, 'online', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![id, owner],
        )?;
        Ok(Node {
            id: id.to_string(),
            owner: owner.to_string(),
            status: "online".to_string(),
            last_heartbeat: Some(Utc::now()),
        })
    }

    pub fn list_nodes(&self) -> Result<Vec<Node>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, owner, status, last_heartbeat FROM nodes ORDER BY id ASC",
        )?;
        let nodes = stmt
            .query_map([], |row| {
                let hb: Option<String> = row.get(3)?;
                Ok(Node {
                    id: row.get(0)?,
                    owner: row.get(1)?,
                    status: row.get(2)?,
                    last_heartbeat: hb.map(|s| Database::parse_ts(&s)),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(nodes)
    }

    pub fn get_node_owner(&self, node_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT owner FROM nodes WHERE id = ?1",
            params![node_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn heartbeat_node(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE nodes SET last_heartbeat = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), status = 'online' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // --- Workers ---

    fn parse_worker_row(row: &rusqlite::Row) -> rusqlite::Result<Worker> {
        let ts: String = row.get(5)?;
        Ok(Worker {
            name: row.get(0)?,
            instructions: row.get(1)?,
            node_id: row.get(2)?,
            status: row.get(3)?,
            registered_by: row.get(4)?,
            created_at: Database::parse_ts(&ts),
        })
    }

    const WORKER_COLS: &str = "name, instructions, node_id, status, registered_by, created_at";

    pub fn register_worker(
        &self,
        group_name: &str,
        name: &str,
        instructions: &str,
        node_id: &str,
        registered_by: &str,
    ) -> Result<Worker> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        // Create agent for the worker (so it has an inbox)
        tx.execute(
            "INSERT OR IGNORE INTO agents (group_name, id) VALUES (?1, ?2)",
            params![group_name, name],
        )?;

        tx.execute(
            "INSERT INTO workers (group_name, name, instructions, node_id, registered_by) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![group_name, name, instructions, node_id, registered_by],
        )?;

        tx.commit()?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM workers WHERE group_name = ?1 AND name = ?2",
            params![group_name, name],
            |row| row.get(0),
        )?;

        Ok(Worker {
            name: name.to_string(),
            instructions: instructions.to_string(),
            node_id: node_id.to_string(),
            status: "active".to_string(),
            registered_by: registered_by.to_string(),
            created_at: Self::parse_ts(&ts),
        })
    }

    pub fn list_workers(&self, group_name: &str) -> Result<Vec<Worker>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM workers WHERE group_name = ?1 ORDER BY created_at ASC", Self::WORKER_COLS),
        )?;
        let workers = stmt
            .query_map(params![group_name], Self::parse_worker_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(workers)
    }

    pub fn get_worker(&self, group_name: &str, name: &str) -> Result<Option<Worker>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            &format!("SELECT {} FROM workers WHERE group_name = ?1 AND name = ?2", Self::WORKER_COLS),
            params![group_name, name],
            Self::parse_worker_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn remove_worker(
        &self,
        group_name: &str,
        name: &str,
        user_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        if !user_id.is_empty() {
            let owner: String = conn
                .query_row(
                    "SELECT registered_by FROM workers WHERE group_name = ?1 AND name = ?2",
                    params![group_name, name],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("worker not found"))?;

            if owner != user_id {
                anyhow::bail!("permission denied: worker was created by someone else");
            }
        }

        let tx = conn.unchecked_transaction()?;

        let deleted = tx.execute(
            "DELETE FROM workers WHERE group_name = ?1 AND name = ?2",
            params![group_name, name],
        )?;
        if deleted == 0 {
            anyhow::bail!("worker not found");
        }

        tx.execute(
            "DELETE FROM agents WHERE group_name = ?1 AND id = ?2",
            params![group_name, name],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn update_worker_instructions(
        &self,
        group_name: &str,
        name: &str,
        instructions: &str,
        user_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        if !user_id.is_empty() {
            let owner: String = conn
                .query_row(
                    "SELECT registered_by FROM workers WHERE group_name = ?1 AND name = ?2",
                    params![group_name, name],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("worker not found"))?;

            if owner != user_id {
                anyhow::bail!("permission denied: worker was created by someone else");
            }
        }

        let updated = conn.execute(
            "UPDATE workers SET instructions = ?3 WHERE group_name = ?1 AND name = ?2",
            params![group_name, name, instructions],
        )?;
        if updated == 0 {
            anyhow::bail!("worker not found");
        }
        Ok(())
    }

    pub fn set_worker_status(
        &self,
        group_name: &str,
        name: &str,
        status: &str,
        user_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        if !user_id.is_empty() {
            let owner: String = conn
                .query_row(
                    "SELECT registered_by FROM workers WHERE group_name = ?1 AND name = ?2",
                    params![group_name, name],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("worker not found"))?;

            if owner != user_id {
                anyhow::bail!("permission denied: worker was created by someone else");
            }
        }

        let updated = conn.execute(
            "UPDATE workers SET status = ?3 WHERE group_name = ?1 AND name = ?2",
            params![group_name, name, status],
        )?;
        if updated == 0 {
            anyhow::bail!("worker not found");
        }
        Ok(())
    }

    /// Get all active workers on a node across ALL groups.
    /// Used by the daemon.
    pub fn get_all_active_workers_for_node(
        &self,
        node_id: &str,
    ) -> Result<Vec<(String, Worker)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT group_name, name, instructions, node_id, status, registered_by, created_at FROM workers WHERE node_id = ?1 AND status = 'active'",
        )?;
        let workers = stmt
            .query_map(params![node_id], |row| {
                let group: String = row.get(0)?;
                let ts: String = row.get(6)?;
                Ok((
                    group,
                    Worker {
                        name: row.get(1)?,
                        instructions: row.get(2)?,
                        node_id: row.get(3)?,
                        status: row.get(4)?,
                        registered_by: row.get(5)?,
                        created_at: Database::parse_ts(&ts),
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(workers)
    }

    pub fn get_worker_logs(
        &self,
        group_name: &str,
        worker_name: &str,
        limit: i64,
    ) -> Result<Vec<InboxMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, from_agent, to_agent, type, content, status, created_at
             FROM inbox_messages
             WHERE group_name = ?1 AND (to_agent = ?2 OR from_agent = ?2)
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let messages = stmt
            .query_map(params![group_name, worker_name, limit], |row| {
                let content_str: Option<String> = row.get(5)?;
                let ts: String = row.get(7)?;
                Ok(InboxMessage {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    from_agent: row.get(2)?,
                    to_agent: row.get(3)?,
                    msg_type: row.get(4)?,
                    content: content_str.and_then(|s| serde_json::from_str(&s).ok()),
                    status: row.get(6)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::new(":memory:").unwrap()
    }

    #[test]
    fn test_bootstrap_admin() {
        let db = test_db();
        let result = db.bootstrap_admin().unwrap();
        assert!(result.is_some());
        let (user, key) = result.unwrap();
        assert!(user.is_admin);
        assert!(key.starts_with("b0_"));

        // Second call returns None
        assert!(db.bootstrap_admin().unwrap().is_none());
    }

    #[test]
    fn test_create_user_gets_personal_group() {
        let db = test_db();
        let (user, _key) = db.create_user("alice", false).unwrap();

        let groups = db.list_groups_for_user(&user.id).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "alice");
    }

    #[test]
    fn test_authenticate() {
        let db = test_db();
        let (_user, key) = db.create_user("alice", false).unwrap();

        let authed = db.authenticate(&key).unwrap();
        assert!(authed.is_some());
        assert_eq!(authed.unwrap().name, "alice");

        let bad = db.authenticate("b0_invalid").unwrap();
        assert!(bad.is_none());
    }

    #[test]
    fn test_group_membership() {
        let db = test_db();
        let (alice, _) = db.create_user("alice", false).unwrap();
        let (bob, _) = db.create_user("bob", false).unwrap();

        // Alice creates shared group
        db.create_group("frontend", &alice.id).unwrap();

        // Add Bob
        db.add_group_member("frontend", &bob.id).unwrap();

        // Both are members
        assert!(db.is_group_member("frontend", &alice.id).unwrap());
        assert!(db.is_group_member("frontend", &bob.id).unwrap());

        // Alice has personal + frontend
        let alice_groups = db.list_groups_for_user(&alice.id).unwrap();
        assert_eq!(alice_groups.len(), 2);

        // Bob has personal + frontend
        let bob_groups = db.list_groups_for_user(&bob.id).unwrap();
        assert_eq!(bob_groups.len(), 2);
    }

    #[test]
    fn test_worker_in_group() {
        let db = test_db();
        let (alice, _) = db.create_user("alice", false).unwrap();

        db.register_worker("alice", "reviewer", "Review code.", "local", &alice.id)
            .unwrap();

        let workers = db.list_workers("alice").unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].name, "reviewer");

        // Not visible in other groups
        let (bob, _) = db.create_user("bob", false).unwrap();
        let workers = db.list_workers("bob").unwrap();
        assert_eq!(workers.len(), 0);

        // But visible in shared group
        db.create_group("team", &alice.id).unwrap();
        db.add_group_member("team", &bob.id).unwrap();
        db.register_worker("team", "shared-reviewer", "Review.", "local", &alice.id)
            .unwrap();

        let team_workers = db.list_workers("team").unwrap();
        assert_eq!(team_workers.len(), 1);
        assert_eq!(team_workers[0].name, "shared-reviewer");
    }

    #[test]
    fn test_node_ownership() {
        let db = test_db();
        let (alice, _) = db.create_user("alice", false).unwrap();
        let (bob, _) = db.create_user("bob", false).unwrap();

        db.register_node("alice-gpu", &alice.id).unwrap();

        let owner = db.get_node_owner("alice-gpu").unwrap();
        assert_eq!(owner, Some(alice.id.clone()));

        // Bob is not the owner
        let owner = db.get_node_owner("alice-gpu").unwrap();
        assert_ne!(owner, Some(bob.id));
    }

    #[test]
    fn test_worker_ownership_permission() {
        let db = test_db();
        let (alice, _) = db.create_user("alice", false).unwrap();
        let (bob, _) = db.create_user("bob", false).unwrap();

        db.create_group("team", &alice.id).unwrap();
        db.add_group_member("team", &bob.id).unwrap();

        db.register_worker("team", "reviewer", "Review.", "local", &alice.id)
            .unwrap();

        // Bob cannot remove Alice's worker
        let result = db.remove_worker("team", "reviewer", &bob.id);
        assert!(result.is_err());

        // Alice can
        db.remove_worker("team", "reviewer", &alice.id).unwrap();
    }

    #[test]
    fn test_inbox_roundtrip() {
        let db = test_db();
        let (alice, _) = db.create_user("alice", false).unwrap();
        db.register_agent("alice", "sender").unwrap();
        db.register_agent("alice", "receiver").unwrap();

        let msg = db
            .send_inbox_message("alice", "t1", "sender", "receiver", "request", Some(&serde_json::json!("hello")))
            .unwrap();
        assert_eq!(msg.msg_type, "request");

        let messages = db.get_inbox_messages("alice", "receiver", Some("unread"), None).unwrap();
        assert_eq!(messages.len(), 1);

        db.ack_inbox_message("alice", &msg.id).unwrap();
        let messages = db.get_inbox_messages("alice", "receiver", Some("unread"), None).unwrap();
        assert_eq!(messages.len(), 0);
    }
}
