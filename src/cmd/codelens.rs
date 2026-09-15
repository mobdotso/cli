use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, Api};
use crate::util::CONTENT_PAGE_SIZE;

#[derive(Subcommand)]
pub enum CodelensCmd {
    /// List accessible code indexes and their source commits
    Repositories,
    /// Index a repository through the active agent's granted GitHub connection
    Index {
        repository: String,
        #[arg(long)]
        connection: String,
    },
    /// Search indexed code
    Search {
        query: String,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        connection: Option<String>,
        #[arg(long, default_value_t = CONTENT_PAGE_SIZE)]
        limit: u32,
    },
    /// Read an indexed file with line numbers
    Read {
        repository: String,
        path: String,
        #[arg(long)]
        connection: Option<String>,
        #[arg(long, default_value_t = 1)]
        start_line: u32,
        #[arg(long, default_value_t = 0)]
        end_line: u32,
    },
}

pub fn run(cmd: CodelensCmd, api: &Api) -> Result<()> {
    match cmd {
        CodelensCmd::Repositories => emit(api.get("/codelens/repositories")?),
        CodelensCmd::Index {
            repository,
            connection,
        } => emit(api.post(
            "/codelens/repositories",
            Some(json!({"repository": repository, "connection_id": connection})),
        )?),
        CodelensCmd::Search {
            query,
            repository,
            connection,
            limit,
        } => {
            let mut params = vec![("q", query), ("limit", limit.to_string())];
            if let Some(repository) = repository {
                params.push(("repository", repository));
            }
            if let Some(connection) = connection {
                params.push(("connection_id", connection));
            }
            emit(api.get_query("/codelens/search", &params)?)
        }
        CodelensCmd::Read {
            repository,
            path,
            connection,
            start_line,
            end_line,
        } => {
            let mut params = vec![
                ("repository", repository),
                ("path", path),
                ("start_line", start_line.to_string()),
                ("end_line", end_line.to_string()),
            ];
            if let Some(connection) = connection {
                params.push(("connection_id", connection));
            }
            emit(api.get_query("/codelens/file", &params)?)
        }
    }
}
