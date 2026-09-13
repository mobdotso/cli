use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::read_line_from_stdin;

/// Connection requests: the expiring links agents (or owners) create so a
/// signed-in owner can authorize a service or supply a credential.
#[derive(Subcommand)]
pub enum ConnectionRequestsCmd {
    /// Create an authorization link for your account
    Create {
        #[arg(long)]
        provider: String,
        /// URL for a custom MCP server
        #[arg(long)]
        server_url: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Show a connection request by its link token
    Get { token: String },
    /// Authorize a connection and print its continuation URL
    ///
    /// For OAuth, open the URL in a browser signed in to mob.so as the same owner.
    Start {
        token: String,
        /// Read an API key from stdin for a remote MCP server
        #[arg(long)]
        api_key_stdin: bool,
        /// Username for Basic authentication, such as an Atlassian account email
        #[arg(long, requires = "api_key_stdin")]
        api_key_username: Option<String>,
        /// Client ID from an X developer app or a custom MCP server's OAuth app
        #[arg(long, conflicts_with = "api_key_stdin")]
        client_id: Option<String>,
        /// Read the OAuth app's client secret from stdin
        #[arg(long, requires = "client_id", conflicts_with = "api_key_stdin")]
        client_secret_stdin: bool,
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
        ConnectionRequestsCmd::Create {
            provider,
            server_url,
            name,
        } => emit(api.post(
            "/connection-requests",
            Some(json!({
                "provider": provider,
                "server_url": server_url.unwrap_or_default(),
                "name": name.unwrap_or_default(),
            })),
        )?),
        ConnectionRequestsCmd::Get { token } => {
            emit(api.get(&format!("/connection-requests/{}", seg(&token)))?)
        }
        ConnectionRequestsCmd::Start {
            token,
            api_key_stdin,
            api_key_username,
            client_id,
            client_secret_stdin,
        } => {
            let body = if api_key_stdin {
                Some(json!({
                    "api_key": read_line_from_stdin("API key")?,
                    "api_key_username": api_key_username.unwrap_or_default(),
                }))
            } else if let Some(client_id) = client_id {
                let client_secret = if client_secret_stdin {
                    read_line_from_stdin("Client secret")?
                } else {
                    String::new()
                };
                Some(json!({ "client_id": client_id, "client_secret": client_secret }))
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
