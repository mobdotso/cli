use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_string, read_json_input, read_line_from_stdin, strings};

#[derive(Subcommand)]
pub enum RuntimeCmd {
    /// Show an agent's runtime, grants, and configuration options
    Get { agent_id: String },
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
        /// github, composio, pipedream, mcp, or secret
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
        RuntimeCmd::Connections(cmd) => run_connections(cmd, api),
        RuntimeCmd::Secrets(cmd) => run_secrets(cmd, api),
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
