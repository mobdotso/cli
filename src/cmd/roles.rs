use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_string, strings};

#[derive(Subcommand)]
pub enum RolesCmd {
    /// Create a role
    Create {
        #[arg(long)]
        mob: String,
        name: String,
    },
    /// Rename a role or set its write limit
    Update {
        #[arg(long)]
        mob: String,
        role_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        writes_per_hour: Option<u32>,
        /// Remove the write limit
        #[arg(long)]
        clear_writes: bool,
    },
    /// Delete a role
    Delete {
        #[arg(long)]
        mob: String,
        role_id: String,
    },
    /// Replace a role's permissions
    SetPermissions {
        #[arg(long)]
        mob: String,
        role_id: String,
        /// Permission names
        permissions: Vec<String>,
    },
    /// Replace a role's channel grants. Each grant is channel_id=r, w, rw, or none
    SetChannels {
        #[arg(long)]
        mob: String,
        role_id: String,
        /// channel_id=rw grants (repeatable)
        #[arg(long = "grant")]
        grants: Vec<String>,
    },
    /// Replace a role's members by handle
    SetMembers {
        #[arg(long)]
        mob: String,
        role_id: String,
        handles: Vec<String>,
    },
    /// Set the role new members join with
    SetDefault {
        #[arg(long)]
        mob: String,
        role_id: String,
    },
    /// Ban an account from this mob and restrict anonymous registration from its IP for 24 hours
    BanMember {
        #[arg(long)]
        mob: String,
        member_id: String,
    },
    /// List banned accounts
    Bans {
        #[arg(long)]
        mob: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Lift an account ban and its associated registration restriction
    UnbanMember {
        #[arg(long)]
        mob: String,
        member_id: String,
    },
    /// Slow a member down for a number of days
    Slowdown {
        #[arg(long)]
        mob: String,
        handle: String,
        #[arg(long, default_value_t = 7)]
        days: u32,
    },
    /// Clear a member's slowdown
    ClearSlowdown {
        #[arg(long)]
        mob: String,
        member_id: String,
    },
}

fn parse_grant(raw: &str) -> Result<Value> {
    let Some((channel_id, access)) = raw.split_once('=') else {
        bail!("Grant \"{raw}\" must look like channel_id=rw (r, w, rw, or none)");
    };
    let (can_read, can_write) = match access {
        "r" => (true, false),
        "w" => (false, true),
        "rw" => (true, true),
        "none" => (false, false),
        other => bail!("Unknown access \"{other}\" in grant \"{raw}\"; use r, w, rw, or none"),
    };
    Ok(json!({
        "channel_id": channel_id,
        "can_read": can_read,
        "can_write": can_write,
    }))
}

pub fn run(cmd: RolesCmd, api: &Api) -> Result<()> {
    match cmd {
        RolesCmd::Create { mob, name } => emit(api.post(
            &format!("/mobs/{}/roles", seg(&mob)),
            Some(json!({ "name": name })),
        )?),
        RolesCmd::Update {
            mob,
            role_id,
            name,
            writes_per_hour,
            clear_writes,
        } => emit(api.patch(
            &format!("/mobs/{}/roles/{}", seg(&mob), seg(&role_id)),
            Some(object(vec![
                ("name", opt_string(&name)),
                (
                    "writes_per_hour",
                    writes_per_hour.map(|v| Value::Number(v.into())),
                ),
                ("clear_writes", Some(Value::Bool(clear_writes))),
            ])),
        )?),
        RolesCmd::Delete { mob, role_id } => {
            emit(api.delete(&format!("/mobs/{}/roles/{}", seg(&mob), seg(&role_id)))?)
        }
        RolesCmd::SetPermissions {
            mob,
            role_id,
            permissions,
        } => emit(api.put(
            &format!("/mobs/{}/roles/{}/permissions", seg(&mob), seg(&role_id)),
            Some(json!({ "permissions": strings(&permissions) })),
        )?),
        RolesCmd::SetChannels {
            mob,
            role_id,
            grants,
        } => {
            let channels = grants
                .iter()
                .map(|grant| parse_grant(grant))
                .collect::<Result<Vec<_>>>()?;
            emit(api.put(
                &format!("/mobs/{}/roles/{}/channels", seg(&mob), seg(&role_id)),
                Some(json!({ "channels": channels })),
            )?)
        }
        RolesCmd::SetMembers {
            mob,
            role_id,
            handles,
        } => emit(api.put(
            &format!("/mobs/{}/roles/{}/members", seg(&mob), seg(&role_id)),
            Some(json!({ "handles": strings(&handles) })),
        )?),
        RolesCmd::SetDefault { mob, role_id } => emit(api.patch(
            &format!("/mobs/{}/default-role", seg(&mob)),
            Some(json!({ "role_id": role_id })),
        )?),
        RolesCmd::BanMember { mob, member_id } => emit(api.put(
            &format!("/mobs/{}/bans/{}", seg(&mob), seg(&member_id)),
            None,
        )?),
        RolesCmd::Bans { mob, limit, offset } => emit(api.get_query(
            &format!("/mobs/{}/bans", seg(&mob)),
            &[("limit", limit.to_string()), ("offset", offset.to_string())],
        )?),
        RolesCmd::UnbanMember { mob, member_id } => {
            emit(api.delete(&format!("/mobs/{}/bans/{}", seg(&mob), seg(&member_id)))?)
        }
        RolesCmd::Slowdown { mob, handle, days } => emit(api.post(
            &format!("/mobs/{}/slowdowns", seg(&mob)),
            Some(json!({ "handle": handle, "duration_days": days })),
        )?),
        RolesCmd::ClearSlowdown { mob, member_id } => emit(api.delete(&format!(
            "/mobs/{}/slowdowns/{}",
            seg(&mob),
            seg(&member_id)
        ))?),
    }
}
