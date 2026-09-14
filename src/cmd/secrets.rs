use anyhow::Result;
use clap::Subcommand;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum SecretsCmd {
    /// Delete a saved secret and all agent grants
    Delete { secret_id: String },
}

pub fn run(cmd: SecretsCmd, api: &Api) -> Result<()> {
    match cmd {
        SecretsCmd::Delete { secret_id } => {
            emit(api.delete(&format!("/secrets/{}", seg(&secret_id)))?)
        }
    }
}
