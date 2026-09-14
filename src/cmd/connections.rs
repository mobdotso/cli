use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::read_line_from_stdin;

/// Use granted services.
#[derive(Subcommand)]
pub enum ConnectionsCmd {
    /// List the active agent's granted connections
    List,
    /// Delete a saved connection and all agent grants
    Delete { connection_id: String },
    /// Discover a service's current tools
    Tools { connection_id: String },
    /// Call a discovered MCP tool
    Call {
        connection_id: String,
        tool: String,
        #[arg(long, default_value = "{}", value_parser = |value: &str| serde_json::from_str::<serde_json::Value>(value))]
        arguments: serde_json::Value,
    },
    /// List repositories available through a GitHub connection
    Repositories { connection_id: String },
    /// Read an advertised MCP App resource
    Resource { connection_id: String, uri: String },
    /// Call a connected provider's REST API
    Request {
        connection_id: String,
        method: String,
        path: String,
        #[arg(long, value_parser = |value: &str| serde_json::from_str::<serde_json::Value>(value))]
        query: Option<serde_json::Value>,
        #[arg(long, value_parser = |value: &str| serde_json::from_str::<serde_json::Value>(value))]
        body: Option<serde_json::Value>,
    },
}

pub fn run_connected(cmd: ConnectionsCmd, api: &Api) -> Result<()> {
    match cmd {
        ConnectionsCmd::List => emit(api.get("/runtime/connections")?),
        ConnectionsCmd::Delete { connection_id } => {
            emit(api.delete(&format!("/connections/{}", seg(&connection_id)))?)
        }
        ConnectionsCmd::Tools { connection_id } => emit(api.get(&format!(
            "/runtime/connections/{}/tools",
            seg(&connection_id)
        ))?),
        ConnectionsCmd::Call {
            connection_id,
            tool,
            arguments,
        } => emit(api.post(
            &format!(
                "/runtime/connections/{}/tools/{}",
                seg(&connection_id),
                seg(&tool)
            ),
            Some(json!({ "arguments": arguments })),
        )?),
        ConnectionsCmd::Repositories { connection_id } => emit(api.get(&format!(
            "/runtime/connections/{}/repositories",
            seg(&connection_id)
        ))?),
        ConnectionsCmd::Resource { connection_id, uri } => emit(api.get_query(
            &format!("/runtime/connections/{}/resources", seg(&connection_id)),
            &[("uri", uri)],
        )?),
        ConnectionsCmd::Request {
            connection_id,
            method,
            path,
            query,
            body,
        } => emit(api.post(
            &format!("/runtime/connections/{}/request", seg(&connection_id)),
            Some(json!({ "method": method, "path": path, "query": query, "body": body })),
        )?),
    }
}

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
    /// Show a connection request
    Get {
        reference: String,
        /// Use the request ID from an inline App
        #[arg(long)]
        by_id: bool,
    },
    /// Authorize a connection and print its continuation URL
    ///
    /// For OAuth, open the URL in a browser signed in to mob.so as the same owner.
    Start {
        reference: String,
        /// Use the request ID from an inline App
        #[arg(long)]
        by_id: bool,
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
        reference: String,
        /// Use the request ID from an inline App
        #[arg(long)]
        by_id: bool,
        /// Secret name, letters, digits, and underscores
        #[arg(long)]
        name: String,
        /// Secret value; omit to type it on stdin
        #[arg(long)]
        value: Option<String>,
        /// Allowed HTTPS hostnames; repeat for each host
        #[arg(long = "domain")]
        allowed_domains: Vec<String>,
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
        ConnectionRequestsCmd::Get { reference, by_id } => {
            emit(api.get(&request_path(&reference, by_id))?)
        }
        ConnectionRequestsCmd::Start {
            reference,
            by_id,
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
                Some(json!({}))
            };
            emit(api.post(&format!("{}/start", request_path(&reference, by_id)), body)?)
        }
        ConnectionRequestsCmd::Secret {
            reference,
            by_id,
            name,
            value,
            allowed_domains,
        } => {
            let value = match value {
                Some(value) => value,
                None => read_line_from_stdin("Secret value")?,
            };
            emit(api.post(
                &format!("{}/secret", request_path(&reference, by_id)),
                Some(json!({ "name": name, "value": value, "allowed_domains": allowed_domains })),
            )?)
        }
    }
}

fn request_path(reference: &str, by_id: bool) -> String {
    format!(
        "/connection-requests/{}{}",
        seg(reference),
        if by_id { "/form" } else { "" }
    )
}
