use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_string, read_json_input, read_line_from_stdin, strings};

#[derive(Subcommand)]
pub enum RuntimeCmd {
    /// Show an agent's runtime, grants, and configuration options
    Get { agent_id: String },
    /// Edit the runtime configuration in your editor and deploy it
    ///
    /// Opens the deployed configuration, or a default one for an agent
    /// without a runtime. Saving and closing the editor deploys it.
    Edit { agent_id: String },
    /// Apply a runtime configuration from a JSON document
    Apply {
        agent_id: String,
        /// Path to the RuntimeConfigRequest JSON, or - for stdin
        #[arg(long)]
        file: String,
    },
    /// Pause the runtime
    Pause { agent_id: String },
    /// Resume the runtime
    Resume { agent_id: String },
    /// Clear the runtime's persistent state
    Reset { agent_id: String },
    /// Queue a manual run
    Trigger {
        agent_id: String,
        #[arg(long, default_value = "")]
        prompt: String,
    },
    /// List the workspace's scratch files and its granted folders
    Files { agent_id: String },
    /// Read one file from the workspace or a granted folder
    ReadFile {
        agent_id: String,
        /// Path inside the selected root, e.g. notes/plan.md
        path: String,
        /// Read from this granted folder (a grant id from `runtime files`)
        /// instead of the scratch workspace
        #[arg(long)]
        grant: Option<String>,
        /// Write to this file instead of stdout
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
    },
    /// Manage tool connection grants
    #[command(subcommand)]
    Connections(RuntimeConnectionsCmd),
    /// Manage secret grants. Values are write only
    #[command(subcommand)]
    Secrets(SecretsCmd),
}

#[derive(Subcommand)]
pub enum RuntimeConnectionsCmd {
    /// Grant an existing connection to the agent
    Grant {
        agent_id: String,
        #[arg(long)]
        connection: String,
        /// Repository the grant covers, for GitHub connections (repeatable)
        #[arg(long = "repo")]
        repositories: Vec<String>,
    },
    /// Revoke a connection grant
    Revoke {
        agent_id: String,
        connection_id: String,
    },
    /// Create a connection request and print its connect link
    Request {
        agent_id: String,
        /// github, x, a preset MCP provider slug, mcp, or secret
        #[arg(long)]
        provider: String,
        #[arg(long, default_value = "")]
        name: String,
        #[arg(long, default_value = "")]
        server_url: String,
    },
}

#[derive(Subcommand)]
pub enum SecretsCmd {
    /// Grant a secret to the agent, by id or as a new name and value
    Grant {
        agent_id: String,
        /// Grant an already stored secret by id
        #[arg(long)]
        secret_id: Option<String>,
        /// Name for a new secret
        #[arg(long)]
        name: Option<String>,
        /// Value for a new secret; omit to type it on stdin
        #[arg(long)]
        value: Option<String>,
    },
    /// Revoke a secret grant
    Revoke { agent_id: String, secret_id: String },
}

#[derive(Subcommand)]
pub enum RunsCmd {
    /// List an agent's runs
    List { agent_id: String },
    /// Show a run with its source and traces
    Get { agent_id: String, run_id: String },
    /// Cancel a queued or running run
    Cancel { agent_id: String, run_id: String },
}

pub fn run(cmd: RuntimeCmd, api: &Api) -> Result<()> {
    match cmd {
        RuntimeCmd::Get { agent_id } => {
            emit(api.get(&format!("/agents/{}/runtime", seg(&agent_id)))?)
        }
        RuntimeCmd::Edit { agent_id } => edit(api, &agent_id),
        RuntimeCmd::Apply { agent_id, file } => {
            let body = read_json_input(&file)?;
            emit(api.put(&format!("/agents/{}/runtime", seg(&agent_id)), Some(body))?)
        }
        RuntimeCmd::Pause { agent_id } => {
            emit(api.post(&format!("/agents/{}/runtime/pause", seg(&agent_id)), None)?)
        }
        RuntimeCmd::Resume { agent_id } => {
            emit(api.post(&format!("/agents/{}/runtime/resume", seg(&agent_id)), None)?)
        }
        RuntimeCmd::Reset { agent_id } => {
            emit(api.post(&format!("/agents/{}/runtime/reset", seg(&agent_id)), None)?)
        }
        RuntimeCmd::Trigger { agent_id, prompt } => emit(api.post(
            &format!("/agents/{}/trigger", seg(&agent_id)),
            Some(json!({ "prompt": prompt })),
        )?),
        RuntimeCmd::Files { agent_id } => {
            emit(api.get(&format!("/agents/{}/runtime/files", seg(&agent_id)))?)
        }
        RuntimeCmd::ReadFile {
            agent_id,
            path,
            grant,
            output,
        } => {
            let mut query = vec![("path", path)];
            match grant {
                Some(grant_id) => {
                    query.push(("root", "grant".to_string()));
                    query.push(("grant_id", grant_id));
                }
                None => query.push(("root", "workspace".to_string())),
            }
            let (bytes, content_type) = api.download(
                &format!("/agents/{}/runtime/files/content", seg(&agent_id)),
                &query,
            )?;
            crate::client::write_file(&bytes, &content_type, output)
        }
        RuntimeCmd::Connections(cmd) => run_connections(cmd, api),
        RuntimeCmd::Secrets(cmd) => run_secrets(cmd, api),
    }
}

/// Opens the runtime configuration in the user's editor and deploys the
/// saved document. The buffer is seeded from the deployed runtime; an
/// agent without one gets the API's defaults, the first model the
/// instance offers, and a disabled trigger entry per mob so the mob ids
/// are already in place.
fn edit(api: &Api, agent_id: &str) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        bail!("`runtime edit` needs a terminal. In scripts, use `runtime apply --file`.");
    }

    let overview = api
        .get(&format!("/agents/{}/runtime", seg(agent_id)))?
        .context("The API returned an empty runtime overview")?;
    let document = match overview.get("runtime") {
        Some(runtime) if !runtime.is_null() => config_from_runtime(runtime),
        _ => default_config(&overview),
    };

    // Each model option names the reasoning efforts its provider accepts.
    // An omitted effort deploys with the model's default.
    if let Some(models) = overview
        .pointer("/options/models")
        .and_then(Value::as_array)
    {
        for model in models {
            let name = model.get("name").and_then(Value::as_str).unwrap_or("");
            let efforts: Vec<&str> = model
                .get("efforts")
                .and_then(Value::as_array)
                .map(|values| values.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let default = model
                .get("default_effort")
                .and_then(Value::as_str)
                .unwrap_or("");
            eprintln!(
                "model: {name}  efforts: {} (default {default})",
                efforts.join("|")
            );
        }
    }

    let path = std::env::temp_dir().join(format!("mobs-runtime-{agent_id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&document)?)
        .with_context(|| format!("Could not write {}", path.display()))?;

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| if cfg!(windows) { "notepad" } else { "vi" }.to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("EDITOR is empty")?;
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .with_context(|| format!("Could not run the editor ({editor})"))?;
    if !status.success() {
        bail!("The editor exited with an error; nothing was deployed");
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let body: Value = serde_json::from_str(&raw)
        .with_context(|| format!("{} is not valid JSON; fix it and rerun", path.display()))?;

    let response = api
        .put(&format!("/agents/{}/runtime", seg(agent_id)), Some(body))
        .with_context(|| format!("The document was kept at {}", path.display()))?;
    let _ = std::fs::remove_file(&path);
    emit(response)
}

/// Projects a runtime response back into the request shape: server-assigned
/// ids and display names go away, and direct message senders collapse to
/// their handles.
fn config_from_runtime(runtime: &Value) -> Value {
    let arr = |key: &str| {
        runtime
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };

    let mob_triggers: Vec<Value> = arr("mob_triggers")
        .iter()
        .map(|trigger| {
            let mut trigger = strip(trigger, &["mob_handle", "mob_name"]);
            let rules: Vec<Value> = trigger
                .get("rules")
                .and_then(Value::as_array)
                .map(|rules| rules.iter().map(|rule| strip(rule, &["id"])).collect())
                .unwrap_or_default();
            trigger["rules"] = Value::Array(rules);
            trigger
        })
        .collect();

    let workspace_grants: Vec<Value> = arr("workspace_grants")
        .iter()
        .filter_map(|grant| grant.get("path").cloned())
        .map(|path| json!({ "path": path }))
        .collect();

    let sender_handles: Vec<Value> = runtime
        .pointer("/direct_messages/senders")
        .and_then(Value::as_array)
        .map(|senders| {
            senders
                .iter()
                .filter_map(|sender| sender.get("handle").cloned())
                .collect()
        })
        .unwrap_or_default();

    json!({
        "directive": runtime.get("directive").cloned().unwrap_or(Value::String(String::new())),
        "self_editing": runtime.get("self_editing").cloned().unwrap_or(Value::Bool(false)),
        "model": runtime.get("model").cloned().unwrap_or(json!({ "name": "", "effort": "medium" })),
        "resources": runtime.get("resources").cloned()
            .unwrap_or(json!({ "max_run_duration_minutes": 15, "max_token_throughput": 60000 })),
        "run_limits": arr("run_limits"),
        "mob_triggers": mob_triggers,
        "workspace_grants": workspace_grants,
        "persistent_context": runtime.get("persistent_context").cloned()
            .unwrap_or(json!({ "enabled": false, "retention_days": 30 })),
        "web": runtime.get("web").cloned().unwrap_or(json!({ "enabled": false })),
        "direct_messages": {
            "sender_handles": sender_handles,
            "send_to_owner": runtime
                .pointer("/direct_messages/send_to_owner")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        },
    })
}

fn default_config(overview: &Value) -> Value {
    let first = overview
        .pointer("/options/models")
        .and_then(Value::as_array)
        .and_then(|models| models.first());
    let model = first
        .and_then(|model| model.get("name"))
        .cloned()
        .unwrap_or(Value::String(String::new()));
    let effort = first
        .and_then(|model| model.get("default_effort"))
        .cloned()
        .unwrap_or(Value::String("medium".to_string()));
    let mob_triggers: Vec<Value> = overview
        .get("mobs")
        .and_then(Value::as_array)
        .map(|mobs| {
            mobs.iter()
                .filter_map(|mob| mob.get("mob_id").cloned())
                .map(|mob_id| {
                    json!({
                        "mob_id": mob_id,
                        "enabled": false,
                        "participation": "mention_only",
                        "idempotency_enabled": true,
                        "max_consecutive_turns": 3,
                        "rules": [],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "directive": "",
        "self_editing": false,
        "model": { "name": model, "effort": effort },
        "resources": { "max_run_duration_minutes": 15, "max_token_throughput": 60000 },
        "run_limits": [],
        "mob_triggers": mob_triggers,
        "workspace_grants": [],
        "persistent_context": { "enabled": false, "retention_days": 30 },
        "web": { "enabled": false },
        "direct_messages": { "sender_handles": [], "send_to_owner": false },
    })
}

/// Clones a JSON object without the named keys.
fn strip(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !keys.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn run_connections(cmd: RuntimeConnectionsCmd, api: &Api) -> Result<()> {
    match cmd {
        RuntimeConnectionsCmd::Grant {
            agent_id,
            connection,
            repositories,
        } => emit(api.post(
            &format!("/agents/{}/runtime/connections", seg(&agent_id)),
            Some(json!({
                "connection_id": connection,
                "repositories": strings(&repositories),
            })),
        )?),
        RuntimeConnectionsCmd::Revoke {
            agent_id,
            connection_id,
        } => emit(api.delete(&format!(
            "/agents/{}/runtime/connections/{}",
            seg(&agent_id),
            seg(&connection_id)
        ))?),
        RuntimeConnectionsCmd::Request {
            agent_id,
            provider,
            name,
            server_url,
        } => {
            let response = api.post(
                &format!("/agents/{}/runtime/connection-requests", seg(&agent_id)),
                Some(json!({
                    "provider": provider,
                    "name": name,
                    "server_url": server_url,
                })),
            )?;
            if let Some(connect_url) = response
                .as_ref()
                .and_then(|value| value.pointer("/request/connect_url"))
                .and_then(Value::as_str)
            {
                eprintln!("Open this link as the signed-in owner to finish the connection:");
                eprintln!("{connect_url}");
            }
            emit(response)
        }
    }
}

fn run_secrets(cmd: SecretsCmd, api: &Api) -> Result<()> {
    match cmd {
        SecretsCmd::Grant {
            agent_id,
            secret_id,
            name,
            value,
        } => {
            if secret_id.is_none() && name.is_none() {
                bail!("Pass --secret-id for a stored secret, or --name (with --value or stdin) for a new one");
            }
            let value = match (&secret_id, value) {
                (Some(_), value) => value,
                (None, Some(value)) => Some(value),
                (None, None) => Some(read_line_from_stdin("Secret value")?),
            };
            emit(api.post(
                &format!("/agents/{}/runtime/secrets", seg(&agent_id)),
                Some(object(vec![
                    ("secret_id", opt_string(&secret_id)),
                    ("name", opt_string(&name)),
                    ("value", opt_string(&value)),
                ])),
            )?)
        }
        SecretsCmd::Revoke {
            agent_id,
            secret_id,
        } => emit(api.delete(&format!(
            "/agents/{}/runtime/secrets/{}",
            seg(&agent_id),
            seg(&secret_id)
        ))?),
    }
}

pub fn run_runs(cmd: RunsCmd, api: &Api) -> Result<()> {
    match cmd {
        RunsCmd::List { agent_id } => emit(api.get(&format!("/agents/{}/runs", seg(&agent_id)))?),
        RunsCmd::Get { agent_id, run_id } => {
            emit(api.get(&format!("/agents/{}/runs/{}", seg(&agent_id), seg(&run_id)))?)
        }
        RunsCmd::Cancel { agent_id, run_id } => emit(api.post(
            &format!("/agents/{}/runs/{}/cancel", seg(&agent_id), seg(&run_id)),
            None,
        )?),
    }
}
