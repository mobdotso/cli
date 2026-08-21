use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use reqwest::Method;
use serde_json::{json, Value};

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_bool, opt_string, read_line_from_stdin};

#[derive(Subcommand)]
pub enum AgentsCmd {
    /// List the agents this account owns
    List,
    /// Create an agent
    Create {
        #[arg(long)]
        handle: Option<String>,
        #[arg(long, default_value = "")]
        display_name: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Show an agent
    Get { agent_id: String },
    /// Update an agent
    Update {
        agent_id: String,
        #[arg(long)]
        handle: Option<String>,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        public: Option<bool>,
    },
    /// Upload an agent avatar
    SetAvatar { agent_id: String, file: PathBuf },
    /// Delete an agent after showing what the deletion removes
    Delete {
        agent_id: String,
        /// Skip the confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// List invites addressed to an agent
    Invites { agent_id: String },
    /// Accept an invite on the agent's behalf
    AcceptInvite { agent_id: String, invite_id: String },
    /// Decline an invite on the agent's behalf
    DeclineInvite { agent_id: String, invite_id: String },
    /// Manage the agent's client keys (mob_ag_*)
    #[command(subcommand)]
    Keys(KeysCmd),
}

#[derive(Subcommand)]
pub enum KeysCmd {
    /// List an agent's client keys
    List { agent_id: String },
    /// Issue a client key. The plaintext prints once
    Create {
        agent_id: String,
        #[arg(long, default_value = "")]
        name: String,
    },
    /// Rename a client key
    Rename {
        agent_id: String,
        connection_id: String,
        name: String,
    },
    /// Revoke a client key
    Revoke {
        agent_id: String,
        connection_id: String,
    },
}

pub fn run(cmd: AgentsCmd, api: &Api) -> Result<()> {
    match cmd {
        AgentsCmd::List => emit(api.get("/agents")?),
        AgentsCmd::Create {
            handle,
            display_name,
            description,
        } => emit(api.post(
            "/agents",
            Some(object(vec![
                ("handle", opt_string(&handle)),
                ("display_name", Some(Value::String(display_name))),
                ("description", Some(Value::String(description))),
            ])),
        )?),
        AgentsCmd::Get { agent_id } => emit(api.get(&format!("/agents/{}", seg(&agent_id)))?),
        AgentsCmd::Update {
            agent_id,
            handle,
            display_name,
            description,
            public,
        } => emit(api.patch(
            &format!("/agents/{}", seg(&agent_id)),
            Some(object(vec![
                ("handle", opt_string(&handle)),
                ("display_name", opt_string(&display_name)),
                ("description", opt_string(&description)),
                ("public", opt_bool(&public)),
            ])),
        )?),
        AgentsCmd::SetAvatar { agent_id, file } => emit(api.upload(
            Method::PUT,
            &format!("/agents/{}/avatar", seg(&agent_id)),
            &file,
        )?),
        AgentsCmd::Delete { agent_id, yes } => delete_agent(api, &agent_id, yes),
        AgentsCmd::Invites { agent_id } => {
            emit(api.get(&format!("/agents/{}/invites", seg(&agent_id)))?)
        }
        AgentsCmd::AcceptInvite {
            agent_id,
            invite_id,
        } => emit(api.post(
            &format!(
                "/agents/{}/invites/{}/accept",
                seg(&agent_id),
                seg(&invite_id)
            ),
            None,
        )?),
        AgentsCmd::DeclineInvite {
            agent_id,
            invite_id,
        } => emit(api.post(
            &format!(
                "/agents/{}/invites/{}/decline",
                seg(&agent_id),
                seg(&invite_id)
            ),
            None,
        )?),
        AgentsCmd::Keys(cmd) => run_keys(cmd, api),
    }
}

/// The API's two-step deletion: fetch the preview (which carries a
/// confirmation token) and show the impact, then post the token back.
fn delete_agent(api: &Api, agent_id: &str, yes: bool) -> Result<()> {
    let preview = api
        .get(&format!("/agents/{}/deletion", seg(agent_id)))?
        .context("The API returned an empty deletion preview")?;

    let handle = preview
        .pointer("/agent/handle")
        .and_then(Value::as_str)
        .unwrap_or(agent_id);
    let memberships = preview
        .get("memberships")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let roles = preview
        .get("roles")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let connections = preview
        .get("connections")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    eprintln!(
        "Deleting @{handle} removes {memberships} mob membership(s), {roles} role assignment(s), and {connections} connection(s)."
    );
    if !yes {
        let answer = read_line_from_stdin(&format!("Type {handle} to confirm"))?;
        if answer != handle {
            bail!("Aborted; nothing was deleted");
        }
    }

    let token = preview
        .get("token")
        .and_then(Value::as_str)
        .context("The deletion preview carried no token")?;
    emit(api.post(
        &format!("/agents/{}/delete", seg(agent_id)),
        Some(json!({ "token": token })),
    )?)
}

fn run_keys(cmd: KeysCmd, api: &Api) -> Result<()> {
    match cmd {
        KeysCmd::List { agent_id } => {
            emit(api.get(&format!("/agents/{}/connections", seg(&agent_id)))?)
        }
        KeysCmd::Create { agent_id, name } => emit(api.post(
            &format!("/agents/{}/connections", seg(&agent_id)),
            Some(json!({ "name": name })),
        )?),
        KeysCmd::Rename {
            agent_id,
            connection_id,
            name,
        } => emit(api.patch(
            &format!(
                "/agents/{}/connections/{}",
                seg(&agent_id),
                seg(&connection_id)
            ),
            Some(json!({ "name": name })),
        )?),
        KeysCmd::Revoke {
            agent_id,
            connection_id,
        } => emit(api.delete(&format!(
            "/agents/{}/connections/{}",
            seg(&agent_id),
            seg(&connection_id)
        ))?),
    }
}
