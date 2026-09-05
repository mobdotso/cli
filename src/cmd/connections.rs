use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::read_line_from_stdin;

/// Connection requests: the expiring links agents (or owners) create so a
/// signed-in owner can authorize a service or supply a credential.
#[derive(Subcommand)]
pub enum ConnectionRequestsCmd {
    /// Show a connection request by its link token
    Get { token: String },
    /// Authorize a connection and print its continuation URL
    Start {
        token: String,
        /// Read a bearer API key from stdin for a remote MCP server
        #[arg(long)]
        api_key_stdin: bool,
    },
    /// Complete a secret request by submitting the value
    Secret {
        token: String,
        /// Secret name, letters, digits, and underscores
        #[arg(long)]
        name: String,
        /// Secret value; omit to type it on stdin
        #[arg(long)]
        value: Option<String>,
    },
}

pub fn run(cmd: ConnectionRequestsCmd, api: &Api) -> Result<()> {
    match cmd {
        ConnectionRequestsCmd::Get { token } => {
            emit(api.get(&format!("/connection-requests/{}", seg(&token)))?)
        }
        ConnectionRequestsCmd::Start {
            token,
            api_key_stdin,
        } => {
            let body = if api_key_stdin {
                Some(json!({ "api_key": read_line_from_stdin("API key")? }))
            } else {
                None
            };
            emit(api.post(&format!("/connection-requests/{}/start", seg(&token)), body)?)
        }
        ConnectionRequestsCmd::Secret { token, name, value } => {
            let value = match value {
                Some(value) => value,
                None => read_line_from_stdin("Secret value")?,
            };
            emit(api.post(
                &format!("/connection-requests/{}/secret", seg(&token)),
                Some(json!({ "name": name, "value": value })),
            )?)
        }
    }
}
