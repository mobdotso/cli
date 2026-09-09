use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;
use serde_json::json;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum AccountCmd {
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
    /// Upload the account's profile picture
    SetAvatar { file: PathBuf },
    /// Unlink a sign-in identity
    Unlink {
        /// Provider: discord, github, or x
        provider: String,
        /// The provider's user id for the identity
        provider_user_id: String,
    },
    /// List the clients authorized on this account through browser
    /// authorization
    Clients,
    /// Revoke an authorized client's tokens; it signs in again through
    /// browser authorization
    RevokeClient { connection_id: String },
}

pub fn run(cmd: AccountCmd, api: &Api) -> Result<()> {
    match cmd {
        AccountCmd::Get => emit(api.get("/account")?),
        AccountCmd::Update { description } => {
            emit(api.patch("/account", Some(json!({ "description": description })))?)
        }
        AccountCmd::SetHandle { handle } => {
            emit(api.put("/account/handle", Some(json!({ "handle": handle })))?)
        }
        AccountCmd::SetAvatar { file } => {
            emit(api.upload(Method::PUT, "/account/avatar", &file)?)
        }
        AccountCmd::Unlink {
            provider,
            provider_user_id,
        } => emit(api.delete(&format!(
            "/account/identities/{}/{}",
            seg(&provider),
            seg(&provider_user_id)
        ))?),
        AccountCmd::Clients => emit(api.get("/oauth/connections")?),
        AccountCmd::RevokeClient { connection_id } => {
            emit(api.delete(&format!("/oauth/connections/{}", seg(&connection_id)))?)
        }
    }
}
