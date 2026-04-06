use box0::{
    client, config,
    db::{AdminEnsureStatus, Database},
    scheduler,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "b0", about = "Box0 agent platform", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the Box0 server
    Server {
        #[command(subcommand)]
        command: Option<ServerCommand>,
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        db: Option<String>,
        #[arg(long)]
        no_local: bool,
        /// Run server in foreground (internal, used by daemonizer)
        #[arg(long, hide = true)]
        foreground: bool,
    },
    /// Create a new agent
    Add {
        /// Agent name
        name: String,
        /// Agent instructions
        #[arg(long)]
        instructions: String,
        /// Short description
        #[arg(long, default_value = "")]
        description: String,
        /// Runtime: auto (default), claude, or codex
        #[arg(long, default_value = "auto")]
        runtime: String,
        /// Run on a schedule (e.g. 30s, 5m, 1h, 6h, 1d)
        #[arg(long)]
        every: Option<String>,
        /// Task prompt to run on schedule (required with --every)
        #[arg(long)]
        task: Option<String>,
        /// Enable webhook trigger and print trigger URL
        #[arg(long)]
        webhook: bool,
        /// HMAC secret for webhook signature verification
        #[arg(long)]
        webhook_secret: Option<String>,
    },
    /// List agents
    Ls,
    /// Show agent details
    Info {
        /// Agent name
        name: String,
    },
    /// Update agent instructions
    Update {
        /// Agent name
        name: String,
        #[arg(long)]
        instructions: String,
    },
    /// Delete an agent
    Rm {
        /// Agent name
        name: String,
    },
    /// Trigger an agent and wait for the result
    Run {
        /// Agent name
        name: String,
        /// Task to run
        task: String,
        /// Timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: f64,
    },
    /// View agent execution logs
    Logs {
        /// Agent name
        name: String,
    },
    /// Reset all local data (dev use)
    #[command(hide = true)]
    Reset,
    /// Install agent skill
    #[command(hide = true)]
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Manage local admin users
    #[command(hide = true)]
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[derive(Subcommand)]
enum ServerCommand {
    /// Stop the running server
    Stop,
    /// Show server status
    Status,
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Create or update a local admin user with a fixed API key
    Ensure {
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        db: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Subcommand)]
enum SkillCommand {
    /// Install skill for Claude Code
    Install {
        /// Agent runtime: claude-code or codex
        agent: String,
    },
}

fn require_config(cfg: &config::CliConfig) {
    if cfg.api_key.is_none() {
        eprintln!("Not connected to a server. Run one of:");
        eprintln!("  b0 server                              Start a local server");
        std::process::exit(1);
    }
}

fn make_client(cfg: &config::CliConfig) -> client::BhClient {
    require_config(cfg);
    match &cfg.api_key {
        Some(key) => client::BhClient::with_api_key(&cfg.server_url(), key),
        None => client::BhClient::new(&cfg.server_url()),
    }
}


fn resolve_workspace() -> String {
    let cfg = config::CliConfig::load();
    if let Some(w) = cfg.default_workspace {
        return w;
    }
    eprintln!("Error: server not configured. Run: b0 server");
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Server {
            command: Some(ServerCommand::Stop),
            ..
        } => {
            cmd_server_stop();
        }
        Command::Server {
            command: Some(ServerCommand::Status),
            ..
        } => {
            cmd_server_status();
        }
        Command::Server {
            command: None,
            config,
            host,
            port,
            db,
            no_local,
            foreground,
        } => {
            if foreground {
                cmd_server_run(config, host, port, db, no_local).await;
            } else {
                cmd_server_start(config, host, port, db, no_local);
            }
        }
        Command::Add {
            name,
            instructions,
            description,
            runtime,
            every,
            task,
            webhook,
            webhook_secret,
        } => {
            cmd_add(
                &name,
                &instructions,
                &description,
                &runtime,
                every.as_deref(),
                task.as_deref(),
                webhook,
                webhook_secret.as_deref(),
            )
            .await;
        }
        Command::Ls => {
            cmd_ls().await;
        }
        Command::Info { name } => {
            cmd_info(&name).await;
        }
        Command::Update { name, instructions } => {
            cmd_update(&name, &instructions).await;
        }
        Command::Rm { name } => {
            cmd_rm(&name).await;
        }
        Command::Run { name, task, timeout } => {
            cmd_run(&name, &task, timeout).await;
        }
        Command::Logs { name } => {
            cmd_logs(&name).await;
        }
        Command::Reset => {
            cmd_reset();
        }
        Command::Skill { command } => match command {
            SkillCommand::Install { agent } => {
                if agent == "claude-code" || agent == "claude" {
                    cmd_skill_install_claude_code().await;
                } else {
                    eprintln!("Unknown runtime: {}. Use: claude-code", agent);
                    std::process::exit(1);
                }
            }
        },
        Command::Admin { command } => match command {
            AdminCommand::Ensure {
                config,
                db,
                name,
                key,
            } => {
                cmd_admin_ensure(
                    config.as_deref(),
                    db.as_deref(),
                    name.as_deref(),
                    key.as_deref(),
                );
            }
        },
    }
}

// --- Server stubs (implemented in Task 4) ---

fn cmd_server_start(
    _config: Option<String>,
    _host: Option<String>,
    _port: Option<u16>,
    _db: Option<String>,
    _no_local: bool,
) {
    eprintln!("Server start not yet implemented.");
    std::process::exit(1);
}

fn cmd_server_stop() {
    eprintln!("Server stop not yet implemented.");
    std::process::exit(1);
}

fn cmd_server_status() {
    eprintln!("Server status not yet implemented.");
    std::process::exit(1);
}

async fn cmd_server_run(
    config_path: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    db: Option<String>,
    no_local: bool,
) {
    let mut cfg = config::ServerConfig::load(config_path.as_deref());
    if let Some(h) = host {
        cfg.host = h;
    }
    if let Some(p) = port {
        cfg.port = p;
    }
    if let Some(d) = db {
        cfg.db_path = d;
    }

    let default_level = if cfg.log_level == "info" {
        "warn"
    } else {
        &cfg.log_level
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level)),
        )
        .init();

    box0::server::run(cfg, no_local).await;
}

// --- Command handlers ---

async fn cmd_add(
    name: &str,
    instructions: &str,
    description: &str,
    runtime: &str,
    every: Option<&str>,
    task: Option<&str>,
    webhook: bool,
    webhook_secret: Option<&str>,
) {
    if every.is_some() && task.is_none() {
        eprintln!("Error: --task is required when --every is set.");
        std::process::exit(1);
    }
    if let Some(sched) = every {
        if scheduler::parse_schedule_secs(sched).is_none() {
            eprintln!(
                "Error: invalid schedule \"{}\". Use formats like: 30s, 5m, 1h, 6h, 1d",
                sched
            );
            std::process::exit(1);
        }
    }

    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);
    let webhook_enabled = webhook || webhook_secret.is_some();

    let a = match client
        .register_agent(
            &workspace,
            name,
            description,
            instructions,
            "local",
            runtime,
            "background",
            None,
            None,
            webhook_secret,
            webhook_enabled,
        )
        .await
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    println!("Agent \"{}\" created.", a.name);

    if webhook_enabled {
        println!(
            "Trigger URL: {}/trigger/{}/{}",
            cfg.server_url(),
            workspace,
            a.name
        );
    }

    if let (Some(sched), Some(task_str)) = (every, task) {
        match client
            .create_cron_job(&workspace, name, sched, task_str, None)
            .await
        {
            Ok(_) => println!("Schedule:    every {}", sched),
            Err(e) => {
                eprintln!("Warning: agent created but failed to set schedule: {}", e);
            }
        }
    }
}

async fn cmd_ls() {
    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);

    let agents = match client.list_agents(&workspace).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if agents.is_empty() {
        println!("No agents.");
        return;
    }

    let crons = client.list_cron_jobs(&workspace).await.unwrap_or_default();
    let cron_map: std::collections::HashMap<String, String> =
        crons.iter().map(|c| (c.agent.clone(), c.schedule.clone())).collect();

    println!("{:<20} {:<22} {:<10} {}", "NAME", "TRIGGERS", "STATUS", "CREATED");
    for a in agents {
        let mut triggers: Vec<String> = Vec::new();
        if let Some(sched) = cron_map.get(&a.name) {
            triggers.push(format!("every {}", sched));
        }
        if a.webhook_enabled {
            triggers.push("webhook".to_string());
        }
        let trig = if triggers.is_empty() {
            "-".to_string()
        } else {
            triggers.join(", ")
        };
        println!(
            "{:<20} {:<22} {:<10} {}",
            a.name,
            trig,
            a.status,
            a.created_at.format("%Y-%m-%d")
        );
    }
}

async fn cmd_info(name: &str) {
    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);

    let a = match client.get_agent(&workspace, name).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: agent \"{}\" not found. {}", name, e);
            std::process::exit(1);
        }
    };

    let crons = client.list_cron_jobs(&workspace).await.unwrap_or_default();
    let cron = crons.iter().find(|c| c.agent == name);

    println!("Name:         {}", a.name);
    println!("Status:       {}", a.status);
    println!("Runtime:      {}", a.runtime);
    println!("Instructions: {}", a.instructions);
    if !a.description.is_empty() {
        println!("Description:  {}", a.description);
    }
    println!(
        "Trigger URL:  {}/trigger/{}/{}",
        cfg.server_url(),
        workspace,
        a.name
    );
    if let Some(c) = cron {
        println!("Schedule:     every {} - {}", c.schedule, c.task);
    }
    if a.webhook_secret.is_some() {
        println!("Webhook:      secret set (HMAC-SHA256)");
    }
    println!("Created:      {}", a.created_at.format("%Y-%m-%d %H:%M:%S"));
}

async fn cmd_update(name: &str, instructions: &str) {
    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);
    match client.update_agent(&workspace, name, instructions).await {
        Ok(_) => println!("Agent \"{}\" updated.", name),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn cmd_rm(name: &str) {
    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);

    // Delete associated cron jobs first
    if let Ok(crons) = client.list_cron_jobs(&workspace).await {
        for c in crons.iter().filter(|c| c.agent == name) {
            client.remove_cron_job(&workspace, &c.id).await.ok();
        }
    }

    match client.remove_agent(&workspace, name).await {
        Ok(_) => println!("Agent \"{}\" deleted.", name),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn cmd_run(_name: &str, _task: &str, _timeout: f64) {
    eprintln!("Not yet implemented.");
    std::process::exit(1);
}

async fn cmd_logs(name: &str) {
    let workspace = resolve_workspace();
    let cfg = config::CliConfig::load();
    let client = make_client(&cfg);
    match client.agent_logs(&workspace, name).await {
        Ok(logs) if logs.is_empty() => println!("No logs for agent \"{}\".", name),
        Ok(logs) => {
            for msg in logs {
                let content = msg
                    .content
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .unwrap_or("(empty)");
                println!(
                    "[{}] {} -> {}: {}",
                    msg.created_at.format("%Y-%m-%d %H:%M:%S"),
                    msg.from_id,
                    msg.to_id,
                    &content[..content.len().min(120)]
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_reset() {
    let b0_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".b0");
    for name in ["b0.db", "b0.db-wal", "b0.db-shm"] {
        let path = b0_dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    let cfg = config::CliConfig::load();
    let _ = cfg.clear();
    println!("Reset complete.");
}

fn cmd_admin_ensure(
    config_path: Option<&str>,
    db_override: Option<&str>,
    name_override: Option<&str>,
    key_override: Option<&str>,
) {
    let mut server_cfg = config::ServerConfig::load(config_path);
    if let Some(db_path) = db_override {
        server_cfg.db_path = db_path.to_string();
    }

    let resolved_name = name_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(server_cfg.admin_name.clone())
        .unwrap_or_else(|| {
            eprintln!(
                "Error: admin name is required. Pass --name or set admin_name / B0_ADMIN_NAME."
            );
            std::process::exit(1);
        });

    let resolved_key = key_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(server_cfg.admin_key.clone())
        .unwrap_or_else(|| {
            eprintln!("Error: admin key is required. Pass --key or set admin_key / B0_ADMIN_KEY.");
            std::process::exit(1);
        });

    if let Some(parent) = std::path::Path::new(&server_cfg.db_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Error: failed to create database directory: {}", e);
            std::process::exit(1);
        }
    }

    let db = match Database::new(&server_cfg.db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    match db.ensure_admin_user(&resolved_name, &resolved_key) {
        Ok(result) => {
            match result.status {
                AdminEnsureStatus::Created => {
                    println!(
                        "Admin user \"{}\" created (ID: {}).",
                        result.user.name, result.user.id
                    )
                }
                AdminEnsureStatus::Updated => {
                    println!(
                        "Admin user \"{}\" updated (ID: {}).",
                        result.user.name, result.user.id
                    )
                }
                AdminEnsureStatus::Unchanged => {
                    println!(
                        "Admin user \"{}\" already matched the requested credentials (ID: {}).",
                        result.user.name, result.user.id
                    )
                }
            }
            println!("  Key: {}", result.key);
            println!();
            println!("Use this key for other services. Existing CLI login was not changed.");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn cmd_skill_install_claude_code() {
    let url = "https://raw.githubusercontent.com/risingwavelabs/skills/main/skills/b0/SKILL.md";
    let client = reqwest::Client::new();
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: failed to download skill: {}", e);
            std::process::exit(1);
        }
    };
    if !resp.status().is_success() {
        eprintln!("Error: failed to download skill (HTTP {})", resp.status());
        std::process::exit(1);
    }
    let content = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: failed to read skill content: {}", e);
            std::process::exit(1);
        }
    };

    let dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("skills")
        .join("b0");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Error: failed to create skill directory: {}", e);
        std::process::exit(1);
    }
    if let Err(e) = std::fs::write(dir.join("SKILL.md"), &content) {
        eprintln!("Error: failed to write skill file: {}", e);
        std::process::exit(1);
    }
    println!("Skill installed for Claude Code.");
}
