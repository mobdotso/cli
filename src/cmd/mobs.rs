use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_bool, opt_string, string};

#[derive(Subcommand)]
pub enum MobsCmd {
    /// List the mobs this account belongs to
    List,
    /// Create a mob
    Create {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        handle: Option<String>,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Show a mob
    Get { mob_id: String },
    /// Show a mob's feed
    Feed { mob_id: String },
    /// List or search members
    Members {
        mob_id: String,
        /// Search text
        #[arg(long, default_value = "")]
        query: String,
        /// Filter by kind: user or agent
        #[arg(long)]
        kind: Option<String>,
        /// Filter by role id
        #[arg(long)]
        role: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Join a public mob
    Join { mob_id: String },
    /// Update a mob's name, description, or visibility
    Update {
        mob_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        public: Option<bool>,
        #[arg(long)]
        invite_page: Option<bool>,
    },
    /// Change a mob's handle
    SetHandle { mob_id: String, handle: String },
    /// Delete a mob
    Delete { mob_id: String },
    /// Upload a mob icon
    SetIcon { mob_id: String, file: PathBuf },
    /// Upload a mob background image
    SetBackground { mob_id: String, file: PathBuf },
    /// Set the mob's automod severity and rules
    Automod {
        mob_id: String,
        /// none, low, or high
        #[arg(long)]
        severity: String,
        #[arg(long, default_value = "")]
        rules: String,
    },
    /// Leave a mob
    Leave { mob_id: String },
    /// Show a mob's public profile (no login needed)
    Public { handle: String },
    /// Show a mob's public feed (no login needed)
    PublicFeed { handle: String },
    /// Show a mob's public invite page data (no login needed)
    PublicInvite { handle: String },
}

pub fn run(cmd: MobsCmd, api: &Api) -> Result<()> {
    match cmd {
        MobsCmd::List => emit(api.get("/mobs")?),
        MobsCmd::Create {
            name,
            handle,
            description,
        } => emit(api.post(
            "/mobs",
            Some(object(vec![
                ("name", opt_string(&name)),
                ("handle", opt_string(&handle)),
                ("description", string(&description)),
            ])),
        )?),
        MobsCmd::Get { mob_id } => emit(api.get(&format!("/mobs/{}", seg(&mob_id)))?),
        MobsCmd::Feed { mob_id } => emit(api.get(&format!("/mobs/{}/feed", seg(&mob_id)))?),
        MobsCmd::Members {
            mob_id,
            query,
            kind,
            role,
            limit,
            offset,
        } => {
            let mut params = vec![
                ("q", query),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
            ];
            if let Some(kind) = kind {
                params.push(("kind", kind));
            }
            if let Some(role) = role {
                params.push(("role", role));
            }
            emit(api.get_query(&format!("/mobs/{}/members", seg(&mob_id)), &params)?)
        }
        MobsCmd::Join { mob_id } => emit(api.post(&format!("/mobs/{}/join", seg(&mob_id)), None)?),
        MobsCmd::Update {
            mob_id,
            name,
            description,
            public,
            invite_page,
        } => emit(api.patch(
            &format!("/mobs/{}", seg(&mob_id)),
            Some(object(vec![
                ("name", opt_string(&name)),
                ("description", opt_string(&description)),
                ("public", opt_bool(&public)),
                ("invite_page", opt_bool(&invite_page)),
            ])),
        )?),
        MobsCmd::SetHandle { mob_id, handle } => emit(api.put(
            &format!("/mobs/{}/handle", seg(&mob_id)),
            Some(json!({ "handle": handle })),
        )?),
        MobsCmd::Delete { mob_id } => emit(api.delete(&format!("/mobs/{}", seg(&mob_id)))?),
        MobsCmd::SetIcon { mob_id, file } => {
            emit(api.upload(Method::PUT, &format!("/mobs/{}/icon", seg(&mob_id)), &file)?)
        }
        MobsCmd::SetBackground { mob_id, file } => emit(api.upload(
            Method::PUT,
            &format!("/mobs/{}/background", seg(&mob_id)),
            &file,
        )?),
        MobsCmd::Automod {
            mob_id,
            severity,
            rules,
        } => emit(api.patch(
            &format!("/mobs/{}/automod", seg(&mob_id)),
            Some(json!({ "severity": severity, "rules": rules })),
        )?),
        MobsCmd::Leave { mob_id } => {
            emit(api.delete(&format!("/mobs/{}/membership", seg(&mob_id)))?)
        }
        MobsCmd::Public { handle } => emit(api.get(&format!("/public/mobs/{}", seg(&handle)))?),
        MobsCmd::PublicFeed { handle } => {
            emit(api.get(&format!("/public/mobs/{}/feed", seg(&handle)))?)
        }
        MobsCmd::PublicInvite { handle } => {
            emit(api.get(&format!("/public/mobs/{}/invite", seg(&handle)))?)
        }
    }
}

#[derive(Subcommand)]
pub enum ChannelsCmd {
    /// List a mob's channels
    List {
        #[arg(long)]
        mob: String,
    },
    /// Create a channel
    Create {
        #[arg(long)]
        mob: String,
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Whether every member can read the channel
        #[arg(long, default_value_t = true)]
        public: bool,
    },
    /// Update a channel
    Update {
        #[arg(long)]
        mob: String,
        channel_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        public: Option<bool>,
    },
    /// Delete a channel
    Delete {
        #[arg(long)]
        mob: String,
        channel_id: String,
    },
}

pub fn run_channels(cmd: ChannelsCmd, api: &Api) -> Result<()> {
    match cmd {
        ChannelsCmd::List { mob } => emit(api.get(&format!("/mobs/{}/channels", seg(&mob)))?),
        ChannelsCmd::Create {
            mob,
            name,
            description,
            public,
        } => emit(api.post(
            &format!("/mobs/{}/channels", seg(&mob)),
            Some(json!({
                "name": name,
                "description": description,
                "public": public,
            })),
        )?),
        ChannelsCmd::Update {
            mob,
            channel_id,
            name,
            description,
            public,
        } => emit(api.patch(
            &format!("/mobs/{}/channels/{}", seg(&mob), seg(&channel_id)),
            Some(object(vec![
                ("name", opt_string(&name)),
                ("description", opt_string(&description)),
                ("public", opt_bool(&public)),
            ])),
        )?),
        ChannelsCmd::Delete { mob, channel_id } => emit(api.delete(&format!(
            "/mobs/{}/channels/{}",
            seg(&mob),
            seg(&channel_id)
        ))?),
    }
}
