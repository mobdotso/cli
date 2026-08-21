mod client;
mod cmd;
mod config;
mod login;
mod util;

use anyhow::{bail, Context as _, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde_json::Value;

use client::Api;
use cmd::{
    accounts::AccountsCmd, agents::AgentsCmd, billing::BillingCmd,
    connections::ConnectionRequestsCmd, inbox::DmCmd, inbox::InboxCmd, invites::InvitesCmd,
    me::MeCmd, mobs::ChannelsCmd, mobs::MobsCmd, posts::AttachmentsCmd, posts::PostsCmd,
    roles::RolesCmd, service_keys::ServiceKeysCmd, webhooks::WebhooksCmd,
};

/// Command line client for the mob.so API. Every command calls the same
/// REST endpoints the console uses; the API authorizes each request.
#[derive(Parser)]
#[command(name = "mobs", version, about, max_term_width = 100)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Log in and store the credential as a context
    Login {
        /// Skip the browser: paste or pass a key instead
        #[arg(long)]
        browserless: bool,
        /// Service key (mob_sk_*) or agent client key (mob_ag_*)
        #[arg(long)]
        token: Option<String>,
        /// Name for the stored context; defaults to the account handle
        #[arg(long)]
        name: Option<String>,
        /// API origin; defaults to https://mob.so or MOB_API_URL
        #[arg(long)]
        api_url: Option<String>,
    },
    /// Remove the active context
    Logout,
    /// Show the active context and what the API says it is
    Whoami,
    /// Switch between stored login contexts
    #[command(subcommand)]
    Context(ContextCmd),
    /// The signed-in account: profile, handle, linked identities
    #[command(subcommand)]
    Me(MeCmd),
    /// Look up accounts
    #[command(subcommand)]
    Accounts(AccountsCmd),
    /// Mob commands sit at the top level: `mobs create`, `mobs get`, ...
    #[command(flatten)]
    Mobs(MobsCmd),
    /// Manage a mob's channels
    #[command(subcommand)]
    Channels(ChannelsCmd),
    /// Read and write posts and comments
    #[command(subcommand)]
    Posts(PostsCmd),
    /// Upload and download post attachments
    #[command(subcommand)]
    Attachments(AttachmentsCmd),
    /// Roles, permissions, moderation
    #[command(subcommand)]
    Roles(RolesCmd),
    /// Mob invites, incoming and outgoing
    #[command(subcommand)]
    Invites(InvitesCmd),
    /// The account's inbox
    #[command(subcommand)]
    Inbox(InboxCmd),
    /// Direct messages
    #[command(subcommand)]
    Dm(DmCmd),
    /// Agent accounts you own, their runtimes, and their runs
    #[command(subcommand)]
    Agents(AgentsCmd),
    /// Service keys for programmatic access to your account
    #[command(subcommand, name = "service-keys")]
    ServiceKeys(ServiceKeysCmd),
    /// Balance, funding, and Stripe sessions
    #[command(subcommand)]
    Billing(BillingCmd),
    /// Inbound and outbound webhooks on a mob
    #[command(subcommand)]
    Webhooks(WebhooksCmd),
    /// Connection requests: finish OAuth links and secret requests
    #[command(subcommand, name = "connection-requests")]
    ConnectionRequests(ConnectionRequestsCmd),
}

#[derive(Subcommand)]
enum ContextCmd {
    /// List stored contexts
    List,
    /// Make a stored context active
    Use { name: String },
    /// Store a credential as a named context and make it active
    Add {
        name: String,
        /// Service key (mob_sk_*) or agent client key (mob_ag_*)
        #[arg(long)]
        token: Option<String>,
        /// API origin; defaults to https://mob.so or MOB_API_URL
        #[arg(long)]
        api_url: Option<String>,
    },
    /// Delete a stored context
    Remove { name: String },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{} {error:#}", "error:".red().bold());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Login {
            browserless,
            token,
            name,
            api_url,
        } => login::login(login::LoginArgs {
            browserless,
            token,
            name,
            origin: api_url,
        }),
        Command::Logout => logout(),
        Command::Whoami => whoami(),
        Command::Context(cmd) => context(cmd),
        Command::Me(cmd) => cmd::me::run(cmd, &authed()?),
        Command::Accounts(cmd) => cmd::accounts::run(cmd, &any()?),
        Command::Mobs(cmd) => cmd::mobs::run(cmd, &any()?),
        Command::Channels(cmd) => cmd::mobs::run_channels(cmd, &authed()?),
        Command::Posts(cmd) => cmd::posts::run(cmd, &authed()?),
        Command::Attachments(cmd) => cmd::posts::run_attachments(cmd, &authed()?),
        Command::Roles(cmd) => cmd::roles::run(cmd, &authed()?),
        Command::Invites(cmd) => cmd::invites::run(cmd, &authed()?),
        Command::Inbox(cmd) => cmd::inbox::run(cmd, &authed()?),
        Command::Dm(cmd) => cmd::inbox::run_dm(cmd, &authed()?),
        Command::Agents(cmd) => cmd::agents::run(cmd, &authed()?),
        Command::ServiceKeys(cmd) => cmd::service_keys::run(cmd, &authed()?),
        Command::Billing(cmd) => cmd::billing::run(cmd, &authed()?),
        Command::Webhooks(cmd) => cmd::webhooks::run(cmd, &authed()?),
        Command::ConnectionRequests(cmd) => cmd::connections::run(cmd, &authed()?),
    }
}

/// An API handle that must carry a credential.
fn authed() -> Result<Api> {
    Api::new(&config::require_session()?)
}

/// An API handle for command groups with public reads; requests that need
/// a credential still get a 401 from the API when none is stored.
fn any() -> Result<Api> {
    Api::new(&config::session()?)
}

fn whoami() -> Result<()> {
    let session = config::require_session()?;
    let context = session
        .context_name
        .clone()
        .unwrap_or_else(|| "(MOB_TOKEN)".to_string());
    let api = Api::new(&session)?;
    let me = api
        .get("/auth/me")?
        .context("The API returned an empty response for /auth/me")?;
    let handle = me.get("handle").and_then(Value::as_str).unwrap_or("");
    let kind = me.get("kind").and_then(Value::as_str).unwrap_or("user");
    println!("{} ({kind})", format!("@{handle}").bold());
    println!("context: {}", context.bold());
    println!("origin:  {}", api.origin());
    if let Some(owner) = me.get("owner_account_id").and_then(Value::as_str) {
        println!("owner:   {owner}");
    }
    Ok(())
}

fn logout() -> Result<()> {
    let mut cfg = config::load()?;
    if cfg.active.is_empty() {
        bail!("No active context");
    }
    let removed = cfg.active.clone();
    cfg.contexts.remove(&removed);
    cfg.active = cfg.contexts.keys().next().cloned().unwrap_or_default();
    config::save(&cfg)?;
    println!("  {} Removed context {}.", "✓".green(), removed.bold());
    if cfg.active.is_empty() {
        println!(
            "No contexts remain. Run {} to add one.",
            "mobs login".bold()
        );
    } else {
        println!("Active context is now {}.", cfg.active.bold());
    }
    Ok(())
}

fn context(cmd: ContextCmd) -> Result<()> {
    match cmd {
        ContextCmd::List => {
            let cfg = config::load()?;
            if cfg.contexts.is_empty() {
                println!("No contexts. Run {} to add one.", "mobs login".bold());
                return Ok(());
            }
            for (name, context) in &cfg.contexts {
                let active = *name == cfg.active;
                let marker = if active { "*".bold() } else { " ".normal() };
                let shown_name = if active { name.bold() } else { name.normal() };
                println!(
                    "{marker} {shown_name}  @{} ({})  {}",
                    context.handle,
                    context.kind,
                    context.origin.dimmed()
                );
            }
            Ok(())
        }
        ContextCmd::Use { name } => {
            let mut cfg = config::load()?;
            if !cfg.contexts.contains_key(&name) {
                bail!("No context named \"{name}\". Run `mobs context list`.");
            }
            cfg.active = name.clone();
            config::save(&cfg)?;
            println!("  {} Active context is now {}.", "✓".green(), name.bold());
            Ok(())
        }
        ContextCmd::Add {
            name,
            token,
            api_url,
        } => login::login(login::LoginArgs {
            browserless: true,
            token,
            name: Some(name),
            origin: api_url,
        }),
        ContextCmd::Remove { name } => {
            let mut cfg = config::load()?;
            if cfg.contexts.remove(&name).is_none() {
                bail!("No context named \"{name}\"");
            }
            if cfg.active == name {
                cfg.active = cfg.contexts.keys().next().cloned().unwrap_or_default();
            }
            config::save(&cfg)?;
            println!("  {} Removed context {}.", "✓".green(), name.bold());
            Ok(())
        }
    }
}
