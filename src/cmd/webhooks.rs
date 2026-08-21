use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_bool, opt_string, strings};

#[derive(Subcommand)]
pub enum WebhooksCmd {
    /// Inbound webhooks: URLs that post into a channel
    #[command(subcommand)]
    Inbound(InboundCmd),
    /// Outbound webhooks: deliveries to your URL on mob events
    #[command(subcommand)]
    Outbound(OutboundCmd),
}

#[derive(Subcommand)]
pub enum InboundCmd {
    /// List a mob's inbound webhooks
    List {
        #[arg(long)]
        mob: String,
    },
    /// Create an inbound webhook. The URL prints once
    Create {
        #[arg(long)]
        mob: String,
        name: String,
        #[arg(long)]
        channel: String,
    },
    /// Rename, retarget, or toggle an inbound webhook
    Update {
        #[arg(long)]
        mob: String,
        webhook_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Rotate an inbound webhook's URL
    Rotate {
        #[arg(long)]
        mob: String,
        webhook_id: String,
    },
    /// Delete an inbound webhook
    Delete {
        #[arg(long)]
        mob: String,
        webhook_id: String,
    },
}

#[derive(Subcommand)]
pub enum OutboundCmd {
    /// List a mob's outbound webhooks
    List {
        #[arg(long)]
        mob: String,
    },
    /// Create an outbound webhook
    Create {
        #[arg(long)]
        mob: String,
        name: String,
        #[arg(long)]
        url: String,
        /// Event to deliver, e.g. post.created (repeatable)
        #[arg(long = "event", required = true)]
        events: Vec<String>,
        /// Limit deliveries to one channel
        #[arg(long)]
        channel: Option<String>,
    },
    /// Update an outbound webhook
    Update {
        #[arg(long)]
        mob: String,
        webhook_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        url: Option<String>,
        /// Replace the delivered events (repeatable)
        #[arg(long = "event")]
        events: Vec<String>,
        #[arg(long)]
        channel: Option<String>,
        /// Deliver events from every channel
        #[arg(long)]
        all_channels: bool,
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Rotate an outbound webhook's signing secret
    Rotate {
        #[arg(long)]
        mob: String,
        webhook_id: String,
    },
    /// Delete an outbound webhook
    Delete {
        #[arg(long)]
        mob: String,
        webhook_id: String,
    },
    /// List an outbound webhook's recent deliveries
    Deliveries {
        #[arg(long)]
        mob: String,
        webhook_id: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
}

pub fn run(cmd: WebhooksCmd, api: &Api) -> Result<()> {
    match cmd {
        WebhooksCmd::Inbound(cmd) => run_inbound(cmd, api),
        WebhooksCmd::Outbound(cmd) => run_outbound(cmd, api),
    }
}

fn run_inbound(cmd: InboundCmd, api: &Api) -> Result<()> {
    match cmd {
        InboundCmd::List { mob } => emit(api.get(&format!("/mobs/{}/webhooks", seg(&mob)))?),
        InboundCmd::Create { mob, name, channel } => emit(api.post(
            &format!("/mobs/{}/webhooks", seg(&mob)),
            Some(json!({ "name": name, "channel_id": channel })),
        )?),
        InboundCmd::Update {
            mob,
            webhook_id,
            name,
            channel,
            enabled,
        } => emit(api.patch(
            &format!("/mobs/{}/webhooks/{}", seg(&mob), seg(&webhook_id)),
            Some(object(vec![
                ("name", opt_string(&name)),
                ("channel_id", opt_string(&channel)),
                ("enabled", opt_bool(&enabled)),
            ])),
        )?),
        InboundCmd::Rotate { mob, webhook_id } => emit(api.post(
            &format!("/mobs/{}/webhooks/{}/rotate", seg(&mob), seg(&webhook_id)),
            None,
        )?),
        InboundCmd::Delete { mob, webhook_id } => emit(api.delete(&format!(
            "/mobs/{}/webhooks/{}",
            seg(&mob),
            seg(&webhook_id)
        ))?),
    }
}

fn run_outbound(cmd: OutboundCmd, api: &Api) -> Result<()> {
    match cmd {
        OutboundCmd::List { mob } => {
            emit(api.get(&format!("/mobs/{}/outbound-webhooks", seg(&mob)))?)
        }
        OutboundCmd::Create {
            mob,
            name,
            url,
            events,
            channel,
        } => emit(api.post(
            &format!("/mobs/{}/outbound-webhooks", seg(&mob)),
            Some(object(vec![
                ("name", Some(json!(name))),
                ("url", Some(json!(url))),
                ("events", Some(strings(&events))),
                ("channel_id", opt_string(&channel)),
            ])),
        )?),
        OutboundCmd::Update {
            mob,
            webhook_id,
            name,
            url,
            events,
            channel,
            all_channels,
            enabled,
        } => {
            let events = if events.is_empty() {
                None
            } else {
                Some(strings(&events))
            };
            emit(api.patch(
                &format!("/mobs/{}/outbound-webhooks/{}", seg(&mob), seg(&webhook_id)),
                Some(object(vec![
                    ("name", opt_string(&name)),
                    ("url", opt_string(&url)),
                    ("events", events),
                    ("channel_id", opt_string(&channel)),
                    ("all_channels", Some(json!(all_channels))),
                    ("enabled", opt_bool(&enabled)),
                ])),
            )?)
        }
        OutboundCmd::Rotate { mob, webhook_id } => emit(api.post(
            &format!(
                "/mobs/{}/outbound-webhooks/{}/rotate",
                seg(&mob),
                seg(&webhook_id)
            ),
            None,
        )?),
        OutboundCmd::Delete { mob, webhook_id } => emit(api.delete(&format!(
            "/mobs/{}/outbound-webhooks/{}",
            seg(&mob),
            seg(&webhook_id)
        ))?),
        OutboundCmd::Deliveries {
            mob,
            webhook_id,
            limit,
            offset,
        } => emit(api.get_query(
            &format!(
                "/mobs/{}/outbound-webhooks/{}/deliveries",
                seg(&mob),
                seg(&webhook_id)
            ),
            &[("limit", limit.to_string()), ("offset", offset.to_string())],
        )?),
    }
}
