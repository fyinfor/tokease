//! Headless companion / debugging tool. Same core as the desktop app.
//!
//!   tokease-cli status
//!   tokease-cli login --email a@b.c --password ***
//!   tokease-cli login            # device-code flow, prints the URL
//!   tokease-cli enable codex
//!   tokease-cli restore codex
//!   tokease-cli backups codex
//!   tokease-cli logout

use clap::{Parser, Subcommand};
use tokease_core::service::LoginPoll;
use tokease_core::{ClientId, Tokease};

#[derive(Parser)]
#[command(name = "tokease-cli", version, about = "Tokease connector CLI (debug/headless)")]
struct Cli {
    /// Override the Tokease server URL for this run.
    #[arg(long, env = "TOKEASE_SERVER_URL")]
    server: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show session and per-client status.
    Status,
    /// Log in (device code by default, or email + password).
    Login {
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        password: Option<String>,
    },
    Logout,
    /// One-click enable Tokease for a client (codex | claude | gemini).
    Enable { client: String },
    /// Restore the pre-Tokease configuration.
    Restore {
        client: String,
        #[arg(long)]
        backup: Option<String>,
    },
    /// List backups for a client.
    Backups { client: String },
    /// Fetch and print /client/config.
    Config,
}

fn parse_client(s: &str) -> ClientId {
    ClientId::parse(s).unwrap_or_else(|| {
        eprintln!("unknown client {s:?}; expected codex | claude | gemini");
        std::process::exit(2)
    })
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    if let Some(s) = &cli.server {
        std::env::set_var("TOKEASE_SERVER_URL", s);
    }
    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> tokease_core::Result<()> {
    let app = Tokease::new()?;
    match cli.cmd {
        Cmd::Status => {
            let s = app.session()?;
            println!("server     : {}", s.server_url);
            println!("logged in  : {}", s.logged_in);
            if let Some(u) = &s.user {
                println!("user       : {} {}", u.id, u.email.clone().unwrap_or_default());
            }
            println!("token      : {} [{:?}]", s.token_preview.unwrap_or_else(|| "-".into()), s.storage_backend);
            println!("data dir   : {}", s.data_dir.display());
            println!();
            for c in app.client_statuses()? {
                println!(
                    "{:<12} installed={:<5} enabled={:<5} available={:<5} base_url={} model={}",
                    c.name,
                    c.installed,
                    c.enabled,
                    c.available,
                    c.current.base_url.unwrap_or_else(|| "-".into()),
                    c.current.model.unwrap_or_else(|| "-".into()),
                );
                if let Some(bin) = &c.binary_path {
                    println!("             binary {} ({})", bin.display(), c.version.clone().unwrap_or_else(|| "version unknown".into()));
                }
                if c.outdated {
                    println!("             ! CLI is older than {} — upgrade it", c.min_version.clone().unwrap_or_default());
                }
                for p in c.problems {
                    println!("             ! {p}");
                }
                for e in c.env_conflicts {
                    println!("             ! env {} is set in {} ({}) and overrides the config file", e.name, e.source, e.preview);
                }
            }
        }
        Cmd::Login { email: Some(email), password } => {
            let password = match password {
                Some(p) => p,
                None => rpassword_fallback()?,
            };
            let s = app.login_password(&email, &password).await?;
            println!("logged in as {:?} (token in {:?})", s.user.map(|u| u.id), s.storage_backend);
        }
        Cmd::Login { email: None, .. } => {
            let start = app.device_start().await?;
            println!("Open this URL and enter code {}:", start.user_code);
            println!("  {}", start.verification_uri_complete.clone().unwrap_or(start.verification_uri.clone()));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(start.expires_in);
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(start.interval.max(1))).await;
                match app.device_poll(&start.device_code).await? {
                    LoginPoll::Pending => {
                        if std::time::Instant::now() > deadline {
                            return Err(tokease_core::Error::Other("device code expired".into()));
                        }
                    }
                    LoginPoll::Authorized { session } => {
                        println!("logged in as {:?} (token in {:?})", session.user.map(|u| u.id), session.storage_backend);
                        break;
                    }
                    LoginPoll::Expired => return Err(tokease_core::Error::Other("device code expired".into())),
                    LoginPoll::Denied => return Err(tokease_core::Error::Other("login denied".into())),
                }
            }
        }
        Cmd::Logout => {
            app.logout()?;
            println!("logged out");
        }
        Cmd::Enable { client } => {
            let s = app.enable(parse_client(&client)).await?;
            println!("{} enabled -> {} (model {})", s.name, s.current.base_url.unwrap_or_default(), s.current.model.unwrap_or_default());
            println!("restore point: backup {}", s.backup_id.unwrap_or_default());
        }
        Cmd::Restore { client, backup } => {
            let s = app.restore(parse_client(&client), backup.as_deref())?;
            println!("{} restored (enabled={})", s.name, s.enabled);
        }
        Cmd::Backups { client } => {
            for b in app.backups(parse_client(&client))? {
                println!("{}  {}  {}", b.id, b.created_at, b.dir.display());
                for f in b.files {
                    println!("    {}", f.display());
                }
            }
        }
        Cmd::Config => {
            let c = app.refresh_platform_config().await?;
            println!("{}", serde_json::to_string_pretty(&c).unwrap());
        }
    }
    Ok(())
}

fn rpassword_fallback() -> tokease_core::Result<String> {
    eprint!("password: ");
    let mut s = String::new();
    std::io::stdin()
        .read_line(&mut s)
        .map_err(|e| tokease_core::Error::Other(e.to_string()))?;
    Ok(s.trim_end_matches(['\n', '\r']).to_string())
}
