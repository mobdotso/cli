use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{read_line_from_stdin, strings};

#[derive(Subcommand)]
pub enum InvitesCmd {
    /// Single-use invitation links for people joining a mob
    #[command(subcommand)]
    Links(InviteLinksCmd),
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

#[derive(Subcommand)]
pub enum InviteLinksCmd {
    /// Create a link that works while the invite page is disabled
    Create {
        #[arg(long)]
        mob: String,
        #[arg(long = "role")]
        roles: Vec<String>,
    },
    /// List link status and history
    List {
        #[arg(long)]
        mob: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Retrieve a pending invitation URL to share again
    Get {
        #[arg(long)]
        mob: String,
        link_id: String,
    },
    /// Revoke a pending link
    Revoke {
        #[arg(long)]
        mob: String,
        link_id: String,
    },
    /// Revoke a pending link and create its replacement with these roles
    Replace {
        #[arg(long)]
        mob: String,
        link_id: String,
        #[arg(long = "role")]
        roles: Vec<String>,
    },
    /// Preview a link; reads the token after #token= from stdin
    Preview,
    /// Join as the current user; reads the invitation token from stdin
    Accept {
        /// Revision returned by the preview you reviewed
        #[arg(long)]
        revision: String,
    },
}

fn links(cmd: InviteLinksCmd, api: &Api) -> Result<()> {
    match cmd {
        InviteLinksCmd::Create { mob, roles } => emit(api.post(
            &format!("/mobs/{}/invite-links", seg(&mob)),
            Some(json!({"role_ids": strings(&roles)})),
        )?),
        InviteLinksCmd::List { mob, limit, offset } => emit(api.get(&format!(
            "/mobs/{}/invite-links?limit={limit}&offset={offset}",
            seg(&mob)
        ))?),
        InviteLinksCmd::Get { mob, link_id } => emit(api.get(&format!(
            "/mobs/{}/invite-links/{}", seg(&mob), seg(&link_id)
        ))?),
        InviteLinksCmd::Revoke { mob, link_id } => emit(api.delete(&format!(
            "/mobs/{}/invite-links/{}",
            seg(&mob),
            seg(&link_id)
        ))?),
        InviteLinksCmd::Replace {
            mob,
            link_id,
            roles,
        } => emit(api.post(
            &format!("/mobs/{}/invite-links/{}/replace", seg(&mob), seg(&link_id)),
            Some(json!({"role_ids": strings(&roles)})),
        )?),
        InviteLinksCmd::Preview => {
            let token = read_line_from_stdin("Invitation token")?;
            emit(api.post(
                "/public/invite-links/resolve",
                Some(json!({"token": token})),
            )?)
        }
        InviteLinksCmd::Accept { revision } => {
            let token = read_line_from_stdin("Invitation token")?;
            emit(api.post(
                "/invites/links/accept",
                Some(json!({"token": token, "revision": revision})),
            )?)
        }
    }
}

pub fn run(cmd: InvitesCmd, api: &Api) -> Result<()> {
    match cmd {
        InvitesCmd::Links(cmd) => links(cmd, api),
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
