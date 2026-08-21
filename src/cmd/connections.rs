use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::read_line_from_stdin;

/// Connection requests: the expiring links agents (or owners) create so a
/// signed-in owner can finish an OAuth handshake or type a secret value.
#[derive(Subcommand)]
pub enum ConnectionRequestsCmd {
    /// Show a connection request by its link token
    Get { token: String },
    /// Start the OAuth flow and print the authorization URL
    Start { token: String },
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
        ConnectionRequestsCmd::Start { token } => {
            emit(api.post(&format!("/connection-requests/{}/start", seg(&token)), None)?)
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
