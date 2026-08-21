use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::strings;

#[derive(Subcommand)]
pub enum InvitesCmd {
    /// List invites addressed to this account
    List,
    /// Accept an invite
    Accept { invite_id: String },
    /// Decline an invite
    Decline { invite_id: String },
    /// Invite an account into a mob
    Create {
        #[arg(long)]
        mob: String,
        /// Handle to invite
        handle: String,
        /// Role id the invite assigns on acceptance (repeatable)
        #[arg(long = "role")]
        roles: Vec<String>,
    },
    /// Revoke a pending invite
    Revoke {
        #[arg(long)]
        mob: String,
        invite_id: String,
    },
}

pub fn run(cmd: InvitesCmd, api: &Api) -> Result<()> {
    match cmd {
        InvitesCmd::List => emit(api.get("/invites")?),
        InvitesCmd::Accept { invite_id } => {
            emit(api.post(&format!("/invites/{}/accept", seg(&invite_id)), None)?)
        }
        InvitesCmd::Decline { invite_id } => {
            emit(api.post(&format!("/invites/{}/decline", seg(&invite_id)), None)?)
        }
        InvitesCmd::Create { mob, handle, roles } => emit(api.post(
            &format!("/mobs/{}/invites", seg(&mob)),
            Some(json!({ "handle": handle, "role_ids": strings(&roles) })),
        )?),
        InvitesCmd::Revoke { mob, invite_id } => {
            emit(api.delete(&format!("/mobs/{}/invites/{}", seg(&mob), seg(&invite_id)))?)
        }
    }
}
