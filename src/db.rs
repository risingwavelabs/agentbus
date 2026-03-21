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
pub struct Agent {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub aliases: Vec<String>,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub status: String,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub key_prefix: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
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
            CREATE TABLE IF NOT EXISTS agents (
                tenant TEXT NOT NULL DEFAULT 'default',
                id TEXT NOT NULL,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                last_seen TEXT,
                PRIMARY KEY (tenant, id)
            );

            CREATE TABLE IF NOT EXISTS agent_aliases (
                tenant TEXT NOT NULL DEFAULT 'default',
                alias TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                PRIMARY KEY (tenant, alias),
                FOREIGN KEY (tenant, agent_id) REFERENCES agents(tenant, id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS inbox_messages (
                id TEXT PRIMARY KEY,
                tenant TEXT NOT NULL DEFAULT 'default',
                thread_id TEXT NOT NULL,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                type TEXT NOT NULL,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'unread',
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS nodes (
                tenant TEXT NOT NULL DEFAULT 'default',
                id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'online',
                last_heartbeat TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (tenant, id)
            );

            CREATE TABLE IF NOT EXISTS workers (
                tenant TEXT NOT NULL DEFAULT 'default',
                name TEXT NOT NULL,
                instructions TEXT NOT NULL,
                node_id TEXT NOT NULL DEFAULT 'local',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (tenant, name)
            );

            CREATE TABLE IF NOT EXISTS api_keys (
                tenant TEXT NOT NULL DEFAULT 'default',
                key TEXT NOT NULL PRIMARY KEY,
                description TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_inbox_tenant_to_status ON inbox_messages(tenant, to_agent, status);
            CREATE INDEX IF NOT EXISTS idx_inbox_tenant_thread ON inbox_messages(tenant, thread_id);
            CREATE INDEX IF NOT EXISTS idx_inbox_tenant_to_thread ON inbox_messages(tenant, to_agent, thread_id);
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

    // --- Agents ---

    pub fn register_agent(
        &self,
        tenant: &str,
        id: &str,
        aliases: Option<&[String]>,
    ) -> Result<Agent> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO agents (tenant, id) VALUES (?1, ?2)",
            params![tenant, id],
        )?;

        if let Some(alias_list) = aliases {
            conn.execute(
                "DELETE FROM agent_aliases WHERE tenant = ?1 AND agent_id = ?2",
                params![tenant, id],
            )?;
            for alias in alias_list {
                conn.execute(
                    "INSERT OR REPLACE INTO agent_aliases (tenant, alias, agent_id) VALUES (?1, ?2, ?3)",
                    params![tenant, alias, id],
                )?;
            }
        }

        let (ts, ls): (String, Option<String>) = conn.query_row(
            "SELECT created_at, last_seen FROM agents WHERE tenant = ?1 AND id = ?2",
            params![tenant, id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let agent_aliases = self.get_aliases_inner(&conn, tenant, id);

        Ok(Agent {
            id: id.to_string(),
            aliases: agent_aliases,
            created_at: Database::parse_ts(&ts),
            last_seen: ls.map(|s| Database::parse_ts(&s)),
        })
    }

    pub fn list_agents(&self, tenant: &str) -> Result<Vec<Agent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, last_seen FROM agents WHERE tenant = ?1 ORDER BY created_at ASC",
        )?;
        let agents: Vec<Agent> = stmt
            .query_map(params![tenant], |row| {
                let ts: String = row.get(1)?;
                let ls: Option<String> = row.get(2)?;
                Ok((row.get::<_, String>(0)?, ts, ls))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(id, ts, ls)| {
                let aliases = self.get_aliases_inner(&conn, tenant, &id);
                Agent {
                    id,
                    aliases,
                    created_at: Database::parse_ts(&ts),
                    last_seen: ls.map(|s| Database::parse_ts(&s)),
                }
            })
            .collect();
        Ok(agents)
    }

    fn get_aliases_inner(&self, conn: &Connection, tenant: &str, agent_id: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT alias FROM agent_aliases WHERE tenant = ?1 AND agent_id = ?2")
            .unwrap();
        stmt.query_map(params![tenant, agent_id], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn resolve_agent(&self, tenant: &str, id_or_alias: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE tenant = ?1 AND id = ?2",
                params![tenant, id_or_alias],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)?;

        if exists {
            return Ok(Some(id_or_alias.to_string()));
        }

        conn.query_row(
            "SELECT agent_id FROM agent_aliases WHERE tenant = ?1 AND alias = ?2",
            params![tenant, id_or_alias],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn delete_agent(&self, tenant: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM agent_aliases WHERE tenant = ?1 AND agent_id = ?2",
            params![tenant, id],
        )?;
        let deleted = conn.execute(
            "DELETE FROM agents WHERE tenant = ?1 AND id = ?2",
            params![tenant, id],
        )?;
        if deleted == 0 {
            anyhow::bail!("agent not found");
        }
        Ok(())
    }

    pub fn update_last_seen(&self, tenant: &str, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET last_seen = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE tenant = ?1 AND id = ?2",
            params![tenant, agent_id],
        )?;
        Ok(())
    }

    // --- Inbox Messages ---

    pub fn send_inbox_message(
        &self,
        tenant: &str,
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
            "INSERT INTO inbox_messages (id, tenant, thread_id, from_agent, to_agent, type, content) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg_id, tenant, thread_id, from, to, msg_type, content_str],
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
        tenant: &str,
        agent_id: &str,
        status: Option<&str>,
        thread_id: Option<&str>,
    ) -> Result<Vec<InboxMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut query =
            "SELECT id, thread_id, from_agent, to_agent, type, content, status, created_at FROM inbox_messages WHERE tenant = ?1 AND to_agent = ?2"
                .to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(tenant.to_string()), Box::new(agent_id.to_string())];
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

    pub fn ack_inbox_message(&self, tenant: &str, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE inbox_messages SET status = 'acked' WHERE id = ?1 AND tenant = ?2 AND status = 'unread'",
            params![message_id, tenant],
        )?;
        if updated == 0 {
            anyhow::bail!("message not found or already acked");
        }
        Ok(())
    }

    pub fn get_thread_messages(
        &self,
        tenant: &str,
        thread_id: &str,
    ) -> Result<Vec<InboxMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, from_agent, to_agent, type, content, status, created_at
             FROM inbox_messages WHERE tenant = ?1 AND thread_id = ?2 ORDER BY created_at ASC",
        )?;
        let messages = stmt
            .query_map(params![tenant, thread_id], |row| {
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

    pub fn register_node(&self, tenant: &str, id: &str) -> Result<Node> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO nodes (tenant, id, status, last_heartbeat) VALUES (?1, ?2, 'online', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![tenant, id],
        )?;
        Ok(Node {
            id: id.to_string(),
            status: "online".to_string(),
            last_heartbeat: Some(Utc::now()),
        })
    }

    pub fn list_nodes(&self, tenant: &str) -> Result<Vec<Node>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, status, last_heartbeat FROM nodes WHERE tenant = ?1 ORDER BY id ASC",
        )?;
        let nodes = stmt
            .query_map(params![tenant], |row| {
                let hb: Option<String> = row.get(2)?;
                Ok(Node {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    last_heartbeat: hb.map(|s| Database::parse_ts(&s)),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(nodes)
    }

    pub fn heartbeat_node(&self, tenant: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE nodes SET last_heartbeat = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), status = 'online' WHERE tenant = ?1 AND id = ?2",
            params![tenant, id],
        )?;
        Ok(())
    }

    pub fn remove_node(&self, tenant: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Check no workers assigned
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM workers WHERE tenant = ?1 AND node_id = ?2",
            params![tenant, id],
            |row| row.get(0),
        )?;
        if count > 0 {
            anyhow::bail!(
                "node has {} worker(s) assigned. Remove or reassign them first.",
                count
            );
        }
        let deleted = conn.execute(
            "DELETE FROM nodes WHERE tenant = ?1 AND id = ?2",
            params![tenant, id],
        )?;
        if deleted == 0 {
            anyhow::bail!("node not found");
        }
        Ok(())
    }

    // --- Workers ---

    pub fn register_worker(
        &self,
        tenant: &str,
        name: &str,
        instructions: &str,
        node_id: &str,
    ) -> Result<Worker> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "INSERT OR IGNORE INTO agents (tenant, id) VALUES (?1, ?2)",
            params![tenant, name],
        )?;

        tx.execute(
            "INSERT INTO workers (tenant, name, instructions, node_id) VALUES (?1, ?2, ?3, ?4)",
            params![tenant, name, instructions, node_id],
        )?;

        tx.commit()?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM workers WHERE tenant = ?1 AND name = ?2",
            params![tenant, name],
            |row| row.get(0),
        )?;

        Ok(Worker {
            name: name.to_string(),
            instructions: instructions.to_string(),
            node_id: node_id.to_string(),
            status: "active".to_string(),
            created_at: Self::parse_ts(&ts),
        })
    }

    pub fn list_workers(&self, tenant: &str) -> Result<Vec<Worker>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, instructions, node_id, status, created_at FROM workers WHERE tenant = ?1 ORDER BY created_at ASC",
        )?;
        let workers = stmt
            .query_map(params![tenant], |row| {
                let ts: String = row.get(4)?;
                Ok(Worker {
                    name: row.get(0)?,
                    instructions: row.get(1)?,
                    node_id: row.get(2)?,
                    status: row.get(3)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(workers)
    }

    pub fn get_worker(&self, tenant: &str, name: &str) -> Result<Option<Worker>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT name, instructions, node_id, status, created_at FROM workers WHERE tenant = ?1 AND name = ?2",
            params![tenant, name],
            |row| {
                let ts: String = row.get(4)?;
                Ok(Worker {
                    name: row.get(0)?,
                    instructions: row.get(1)?,
                    node_id: row.get(2)?,
                    status: row.get(3)?,
                    created_at: Database::parse_ts(&ts),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn remove_worker(&self, tenant: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let deleted = tx.execute(
            "DELETE FROM workers WHERE tenant = ?1 AND name = ?2",
            params![tenant, name],
        )?;
        if deleted == 0 {
            anyhow::bail!("worker not found");
        }

        tx.execute(
            "DELETE FROM agent_aliases WHERE tenant = ?1 AND agent_id = ?2",
            params![tenant, name],
        )?;
        tx.execute(
            "DELETE FROM agents WHERE tenant = ?1 AND id = ?2",
            params![tenant, name],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_active_workers_for_node(
        &self,
        tenant: &str,
        node_id: &str,
    ) -> Result<Vec<Worker>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, instructions, node_id, status, created_at FROM workers WHERE tenant = ?1 AND node_id = ?2 AND status = 'active'",
        )?;
        let workers = stmt
            .query_map(params![tenant, node_id], |row| {
                let ts: String = row.get(4)?;
                Ok(Worker {
                    name: row.get(0)?,
                    instructions: row.get(1)?,
                    node_id: row.get(2)?,
                    status: row.get(3)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(workers)
    }

    // --- API Keys ---

    pub fn create_api_key(&self, tenant: &str, description: &str) -> Result<ApiKey> {
        let conn = self.conn.lock().unwrap();
        let key = format!("bh_{}", &Uuid::new_v4().to_string().replace('-', ""));

        conn.execute(
            "INSERT INTO api_keys (tenant, key, description) VALUES (?1, ?2, ?3)",
            params![tenant, key, description],
        )?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM api_keys WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )?;

        Ok(ApiKey {
            key_prefix: key[..12].to_string(),
            description: description.to_string(),
            created_at: Self::parse_ts(&ts),
        })
    }

    /// Returns the full key (only on creation). Caller must show it to the user.
    pub fn create_api_key_full(&self, tenant: &str, description: &str) -> Result<(String, ApiKey)> {
        let conn = self.conn.lock().unwrap();
        let key = format!("bh_{}", &Uuid::new_v4().to_string().replace('-', ""));

        conn.execute(
            "INSERT INTO api_keys (tenant, key, description) VALUES (?1, ?2, ?3)",
            params![tenant, key, description],
        )?;

        let ts: String = conn.query_row(
            "SELECT created_at FROM api_keys WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )?;

        let api_key = ApiKey {
            key_prefix: key[..12].to_string(),
            description: description.to_string(),
            created_at: Self::parse_ts(&ts),
        };
        Ok((key, api_key))
    }

    pub fn list_api_keys(&self, tenant: &str) -> Result<Vec<ApiKey>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, description, created_at FROM api_keys WHERE tenant = ?1 ORDER BY created_at ASC",
        )?;
        let keys = stmt
            .query_map(params![tenant], |row| {
                let key: String = row.get(0)?;
                let ts: String = row.get(2)?;
                Ok(ApiKey {
                    key_prefix: key[..std::cmp::min(12, key.len())].to_string(),
                    description: row.get(1)?,
                    created_at: Database::parse_ts(&ts),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    pub fn revoke_api_key(&self, tenant: &str, key_prefix: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM api_keys WHERE tenant = ?1 AND key LIKE ?2",
            params![tenant, format!("{}%", key_prefix)],
        )?;
        if deleted == 0 {
            anyhow::bail!("API key not found");
        }
        Ok(())
    }

    /// Validate an API key. Returns the tenant if valid.
    pub fn validate_api_key(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT tenant FROM api_keys WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    /// Check if any API keys exist (for deciding whether to enforce auth).
    pub fn has_api_keys(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM api_keys", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|c| c > 0)
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::new(":memory:").unwrap()
    }

    #[test]
    fn test_register_agent() {
        let db = test_db();
        let agent = db.register_agent("default", "test-agent", None).unwrap();
        assert_eq!(agent.id, "test-agent");
    }

    #[test]
    fn test_register_agent_idempotent() {
        let db = test_db();
        db.register_agent("default", "test-agent", None).unwrap();
        let agent = db.register_agent("default", "test-agent", None).unwrap();
        assert_eq!(agent.id, "test-agent");
    }

    #[test]
    fn test_inbox_roundtrip() {
        let db = test_db();
        db.register_agent("default", "sender", None).unwrap();
        db.register_agent("default", "receiver", None).unwrap();

        let msg = db
            .send_inbox_message(
                "default",
                "thread-1",
                "sender",
                "receiver",
                "request",
                Some(&serde_json::json!("hello")),
            )
            .unwrap();
        assert_eq!(msg.msg_type, "request");

        let messages = db
            .get_inbox_messages("default", "receiver", Some("unread"), None)
            .unwrap();
        assert_eq!(messages.len(), 1);

        db.ack_inbox_message("default", &msg.id).unwrap();

        let messages = db
            .get_inbox_messages("default", "receiver", Some("unread"), None)
            .unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_thread_messages() {
        let db = test_db();
        db.register_agent("default", "a", None).unwrap();
        db.register_agent("default", "b", None).unwrap();

        db.send_inbox_message("default", "t1", "a", "b", "request", None)
            .unwrap();
        db.send_inbox_message("default", "t1", "b", "a", "done", None)
            .unwrap();

        let msgs = db.get_thread_messages("default", "t1").unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].msg_type, "request");
        assert_eq!(msgs[1].msg_type, "done");
    }

    #[test]
    fn test_register_worker() {
        let db = test_db();
        let worker = db
            .register_worker("default", "reviewer", "Review code.", "local")
            .unwrap();
        assert_eq!(worker.name, "reviewer");
        assert_eq!(worker.node_id, "local");

        let agent = db.resolve_agent("default", "reviewer").unwrap();
        assert_eq!(agent, Some("reviewer".to_string()));
    }

    #[test]
    fn test_list_workers() {
        let db = test_db();
        db.register_worker("default", "reviewer", "Review code.", "local")
            .unwrap();
        db.register_worker("default", "tester", "Run tests.", "local")
            .unwrap();

        let workers = db.list_workers("default").unwrap();
        assert_eq!(workers.len(), 2);
    }

    #[test]
    fn test_remove_worker() {
        let db = test_db();
        db.register_worker("default", "reviewer", "Review code.", "local")
            .unwrap();
        db.remove_worker("default", "reviewer").unwrap();

        let workers = db.list_workers("default").unwrap();
        assert_eq!(workers.len(), 0);

        let agent = db.resolve_agent("default", "reviewer").unwrap();
        assert_eq!(agent, None);
    }

    #[test]
    fn test_remove_worker_not_found() {
        let db = test_db();
        let result = db.remove_worker("default", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_nodes() {
        let db = test_db();
        db.register_node("default", "gpu-box").unwrap();
        db.register_node("default", "cpu-1").unwrap();

        let nodes = db.list_nodes("default").unwrap();
        assert_eq!(nodes.len(), 2);

        db.heartbeat_node("default", "gpu-box").unwrap();

        db.remove_node("default", "cpu-1").unwrap();
        let nodes = db.list_nodes("default").unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_node_with_workers_cannot_remove() {
        let db = test_db();
        db.register_node("default", "gpu-box").unwrap();
        db.register_worker("default", "ml", "ML agent.", "gpu-box")
            .unwrap();

        let result = db.remove_node("default", "gpu-box");
        assert!(result.is_err());
    }

    #[test]
    fn test_workers_for_node() {
        let db = test_db();
        db.register_node("default", "local").unwrap();
        db.register_node("default", "gpu-box").unwrap();

        db.register_worker("default", "reviewer", "Review.", "local")
            .unwrap();
        db.register_worker("default", "ml", "ML.", "gpu-box")
            .unwrap();

        let local = db.get_active_workers_for_node("default", "local").unwrap();
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].name, "reviewer");

        let gpu = db
            .get_active_workers_for_node("default", "gpu-box")
            .unwrap();
        assert_eq!(gpu.len(), 1);
        assert_eq!(gpu[0].name, "ml");
    }

    #[test]
    fn test_api_keys() {
        let db = test_db();
        let (full_key, key) = db
            .create_api_key_full("default", "test key")
            .unwrap();
        assert!(full_key.starts_with("bh_"));
        assert_eq!(key.description, "test key");

        let keys = db.list_api_keys("default").unwrap();
        assert_eq!(keys.len(), 1);

        let tenant = db.validate_api_key(&full_key).unwrap();
        assert_eq!(tenant, Some("default".to_string()));

        let invalid = db.validate_api_key("bh_invalid").unwrap();
        assert_eq!(invalid, None);

        db.revoke_api_key("default", &key.key_prefix).unwrap();
        let keys = db.list_api_keys("default").unwrap();
        assert_eq!(keys.len(), 0);
    }
}
