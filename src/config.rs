use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// --- Server Config ---

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub slack_token: Option<String>,
    #[serde(default)]
    pub admin_name: Option<String>,
    #[serde(default)]
    pub admin_key: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_db_path() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b0")
        .join("b0.db")
        .to_string_lossy()
        .to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: default_host(),
            port: default_port(),
            db_path: default_db_path(),
            log_level: default_log_level(),
            slack_token: None,
            admin_name: None,
            admin_key: None,
        }
    }
}

impl ServerConfig {
    pub fn load(path: Option<&str>) -> Self {
        let mut cfg = match path {
            Some(p) => match fs::read_to_string(p) {
                Ok(data) => toml::from_str(&data).unwrap_or_else(|e| {
                    eprintln!("failed to parse config: {}", e);
                    ServerConfig::default()
                }),
                Err(_) => ServerConfig::default(),
            },
            None => ServerConfig::default(),
        };

        if let Ok(v) = std::env::var("B0_HOST") {
            if !v.is_empty() {
                cfg.host = v;
            }
        }
        if let Ok(v) = std::env::var("B0_PORT") {
            if let Ok(port) = v.parse::<u16>() {
                cfg.port = port;
            }
        }
        if let Ok(v) = std::env::var("B0_DB_PATH") {
            if !v.is_empty() {
                cfg.db_path = v;
            }
        }
        if let Ok(v) = std::env::var("B0_LOG_LEVEL") {
            if !v.is_empty() {
                cfg.log_level = v;
            }
        }
        if let Ok(v) = std::env::var("B0_SLACK_TOKEN") {
            if !v.is_empty() {
                cfg.slack_token = Some(v);
            }
        }
        if let Ok(v) = std::env::var("B0_ADMIN_NAME") {
            if !v.is_empty() {
                cfg.admin_name = Some(v);
            }
        }
        if let Ok(v) = std::env::var("B0_ADMIN_KEY") {
            if !v.is_empty() {
                cfg.admin_key = Some(v);
            }
        }

        cfg
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// --- CLI Config ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default)]
    pub lead_id: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub default_workspace: Option<String>,
}

fn default_server_url() -> String {
    "http://localhost:8080".to_string()
}

impl Default for CliConfig {
    fn default() -> Self {
        CliConfig {
            server_url: default_server_url(),
            lead_id: None,
            api_key: None,
            default_workspace: None,
        }
    }
}

impl CliConfig {
    fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".b0")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    fn pending_path() -> PathBuf {
        Self::config_dir().join("pending.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(data) => toml::from_str(&data).unwrap_or_default(),
            Err(_) => CliConfig::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        let data = toml::to_string_pretty(self)?;
        fs::write(Self::config_path(), data)?;
        Ok(())
    }

    /// Get or create a stable lead ID.
    pub fn lead_id(&mut self) -> String {
        if let Some(ref id) = self.lead_id {
            return id.clone();
        }
        let id = format!("lead-{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.lead_id = Some(id.clone());
        let _ = self.save();
        id
    }

    /// Get the server URL, with env var override.
    pub fn server_url(&self) -> String {
        if let Ok(v) = std::env::var("B0_SERVER_URL") {
            if !v.is_empty() {
                return v;
            }
        }
        self.server_url.clone()
    }

    /// Acquire an exclusive file lock for pending.json operations.
    /// Hold the returned guard across load + modify + save to prevent races.
    pub fn lock_pending() -> PendingLock {
        let dir = Self::config_dir();
        let _ = fs::create_dir_all(&dir);
        let lock_path = dir.join("pending.lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(lock_path)
            .expect("failed to open pending lock file");
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            unsafe {
                let mut flock: libc::flock = std::mem::zeroed();
                flock.l_type = libc::F_WRLCK as i16;
                flock.l_whence = libc::SEEK_SET as i16;
                libc::fcntl(fd, libc::F_SETLKW, &flock);
            }
        }
        PendingLock { _file: file }
    }

    pub fn load_pending() -> PendingState {
        let path = Self::pending_path();
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => PendingState::default(),
        }
    }

    pub fn save_pending(state: &PendingState) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        let data = serde_json::to_string_pretty(state)?;
        fs::write(Self::pending_path(), data)?;
        Ok(())
    }

    pub fn clear(self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if config_path.exists() {
            fs::remove_file(&config_path)?;
        }
        let pending_path = Self::pending_path();
        if pending_path.exists() {
            fs::remove_file(&pending_path)?;
        }
        Ok(())
    }
}

// --- Pending State ---

/// File lock guard for pending.json. Lock is released when dropped.
pub struct PendingLock {
    _file: std::fs::File,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingState {
    #[serde(default)]
    pub threads: std::collections::HashMap<String, PendingThread>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingThread {
    pub agent: String,
    pub workspace: String,
    pub task: String,
    pub created_at: String,
}
