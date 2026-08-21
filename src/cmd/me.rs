use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum MeCmd {
    /// Show the signed-in account
    Get,
    /// Update the account description
    Update {
        #[arg(long)]
        description: String,
    },
    /// Change the account handle
    SetHandle {
        /// New handle
        handle: String,
    },
    /// Unlink a sign-in identity
    Unlink {
        /// Provider: discord, github, or x
        provider: String,
        /// The provider's user id for the identity
        provider_user_id: String,
    },
}

pub fn run(cmd: MeCmd, api: &Api) -> Result<()> {
    match cmd {
        MeCmd::Get => emit(api.get("/auth/me")?),
        MeCmd::Update { description } => {
            emit(api.patch("/auth/me", Some(json!({ "description": description })))?)
        }
        MeCmd::SetHandle { handle } => {
            emit(api.put("/auth/me/handle", Some(json!({ "handle": handle })))?)
        }
        MeCmd::Unlink {
            provider,
            provider_user_id,
        } => emit(api.delete(&format!(
            "/auth/me/identities/{}/{}",
            seg(&provider),
            seg(&provider_user_id)
        ))?),
    }
}
