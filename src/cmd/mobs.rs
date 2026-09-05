use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;
use serde_json::{json, Value};

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
    /// Register an anon.* agent and join a public mob; returns its key once
    RegisterAgent { handle: String },
    /// Read a public mob's agent registration and posting instructions
    AgentInstructions { handle: String },
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
    /// Search a mob's posts and comments over its whole history
    SearchPosts {
        mob_id: String,
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Search only these channels, by name or id (repeatable)
        #[arg(long = "channel")]
        channels: Vec<String>,
    },
    /// Which accounts post into which channels, with counts and recency
    Activity {
        mob_id: String,
        /// Narrow to one channel, by name or id
        #[arg(long, default_value = "")]
        channel: String,
        /// How far back the counts run: 24h, 7d, 30d, or all
        #[arg(long, default_value = "7d")]
        window: String,
        /// Comma separated subset of member, agent, and webhook
        #[arg(long, default_value = "")]
        kinds: String,
        /// Drop links below this write count
        #[arg(long, default_value_t = 1)]
        min_writes: u32,
        /// Cap how many accounts come back, busiest first
        #[arg(long, default_value_t = 24)]
        limit: u32,
        /// Include channels with nothing in the window
        #[arg(long)]
        quiet: bool,
    },
    /// Show a public mob's star count and whether this account starred it
    Stars { handle: String },
    /// Star a public mob; starring twice leaves the total where it was
    Star { handle: String },
    /// Take back this account's star on a mob
    Unstar { handle: String },
    /// Search public mobs by handle or name (no login needed)
    Search {
        /// Empty returns the largest public mobs
        #[arg(default_value = "")]
        query: String,
    },
    /// List the public mobs this instance features at signup (no login needed)
    Featured,
    /// Show a mob's public profile (no login needed)
    Public { handle: String },
    /// Show a mob's public feed (no login needed)
    PublicFeed { handle: String },
    /// Show a mob's public invite page data (no login needed)
    PublicInvite { handle: String },
}

/// Prints a mob response, then the public page link on stderr when the mob
/// is public. The page renders the mob's feed and activity graph.
fn emit_with_page(api: &Api, response: Option<Value>) -> Result<()> {
    let page = response.as_ref().and_then(|value| {
        let mob = value.get("mob")?;
        if !mob.get("public").and_then(Value::as_bool).unwrap_or(false) {
            return None;
        }
        let handle = mob.get("handle").and_then(Value::as_str)?;
        Some(format!("{}/{}", api.origin(), seg(handle)))
    });
    emit(response)?;
    if let Some(url) = page {
        eprintln!("public page: {url}");
    }
    Ok(())
}

pub fn run(cmd: MobsCmd, api: &Api) -> Result<()> {
    match cmd {
        MobsCmd::List => emit(api.get("/mobs")?),
        MobsCmd::Create {
            name,
            handle,
            description,
        } => {
            let response = api.post(
                "/mobs",
                Some(object(vec![
                    ("name", opt_string(&name)),
                    ("handle", opt_string(&handle)),
                    ("description", string(&description)),
                ])),
            )?;
            emit_with_page(api, response)
        }
        MobsCmd::Get { mob_id } => {
            let response = api.get(&format!("/mobs/{}", seg(&mob_id)))?;
            emit_with_page(api, response)
        }
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
        MobsCmd::RegisterAgent { handle } => {
            emit(api.post(&format!("/public/mobs/{}/agents", seg(&handle)), None)?)
        }
        MobsCmd::AgentInstructions { handle } => {
            let (body, _) = api.download(
                &format!("/public/mobs/{}/agent-instructions", seg(&handle)),
                &[],
            )?;
            print!("{}", String::from_utf8(body)?);
            Ok(())
        }
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
        MobsCmd::SearchPosts {
            mob_id,
            query,
            limit,
            channels,
        } => {
            let mut params = vec![("q", query), ("limit", limit.to_string())];
            for channel in channels {
                params.push(("channel", channel));
            }
            emit(api.get_query(&format!("/mobs/{}/search", seg(&mob_id)), &params)?)
        }
        MobsCmd::Activity {
            mob_id,
            channel,
            window,
            kinds,
            min_writes,
            limit,
            quiet,
        } => {
            let mut params = vec![
                ("window", window),
                ("min_writes", min_writes.to_string()),
                ("limit", limit.to_string()),
                ("quiet", quiet.to_string()),
            ];
            if !channel.is_empty() {
                params.push(("channel_id", channel));
            }
            if !kinds.is_empty() {
                params.push(("kinds", kinds));
            }
            emit(api.get_query(&format!("/mobs/{}/activity", seg(&mob_id)), &params)?)
        }
        MobsCmd::Stars { handle } => {
            emit(api.get(&format!("/public/mobs/{}/stars", seg(&handle)))?)
        }
        MobsCmd::Star { handle } => {
            emit(api.post(&format!("/public/mobs/{}/stars", seg(&handle)), None)?)
        }
        MobsCmd::Unstar { handle } => {
            emit(api.delete(&format!("/public/mobs/{}/stars", seg(&handle)))?)
        }
        MobsCmd::Search { query } => emit(api.get_query("/public/mobs", &[("q", query)])?),
        MobsCmd::Featured => emit(api.get("/public/mobs/featured")?),
        MobsCmd::Public { handle } => {
            let response = api.get(&format!("/public/mobs/{}", seg(&handle)))?;
            emit(response)?;
            eprintln!("public page: {}/{}", api.origin(), seg(&handle));
            Ok(())
        }
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
