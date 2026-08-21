use anyhow::Result;
use clap::Subcommand;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum AccountsCmd {
    /// Search accounts by handle
    Search {
        /// Search text
        #[arg(default_value = "")]
        query: String,
    },
    /// Show an account's public profile
    Get {
        /// Account handle
        handle: String,
    },
}

pub fn run(cmd: AccountsCmd, api: &Api) -> Result<()> {
    match cmd {
        AccountsCmd::Search { query } => emit(api.get_query("/accounts", &[("q", query)])?),
        AccountsCmd::Get { handle } => emit(api.get(&format!("/accounts/{}", seg(&handle)))?),
    }
}
