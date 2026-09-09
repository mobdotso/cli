use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{object, opt_bool};

#[derive(Subcommand)]
pub enum InboxCmd {
    /// List inbox entries
    List {
        /// Show archived entries instead
        #[arg(long)]
        archived: bool,
        #[arg(long, default_value_t = 25, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        /// next_cursor from the previous response
        #[arg(long, default_value = "")]
        cursor: String,
    },
    /// Show one inbox entry
    Get { entry_id: String },
    /// Mark an entry read or archived
    Update {
        entry_id: String,
        #[arg(long)]
        read: Option<bool>,
        #[arg(long)]
        archived: Option<bool>,
    },
    /// Accept the invite behind an inbox entry
    Accept { entry_id: String },
    /// Decline the invite behind an inbox entry
    Decline { entry_id: String },
}

pub fn run(cmd: InboxCmd, api: &Api) -> Result<()> {
    match cmd {
        InboxCmd::List {
            archived,
            limit,
            cursor,
        } => emit(api.get_query(
            "/inbox",
            &[
                ("archived", archived.to_string()),
                ("limit", limit.to_string()),
                ("cursor", cursor),
            ],
        )?),
        InboxCmd::Get { entry_id } => emit(api.get(&format!("/inbox/{}", seg(&entry_id)))?),
        InboxCmd::Update {
            entry_id,
            read,
            archived,
        } => emit(api.patch(
            &format!("/inbox/{}", seg(&entry_id)),
            Some(object(vec![
                ("read", opt_bool(&read)),
                ("archived", opt_bool(&archived)),
            ])),
        )?),
        InboxCmd::Accept { entry_id } => {
            emit(api.post(&format!("/inbox/{}/accept", seg(&entry_id)), None)?)
        }
        InboxCmd::Decline { entry_id } => {
            emit(api.post(&format!("/inbox/{}/decline", seg(&entry_id)), None)?)
        }
    }
}

#[derive(Subcommand)]
pub enum DmCmd {
    /// Send a direct message
    Send {
        /// Recipient handle
        handle: String,
        /// Message body
        body: String,
    },
}

pub fn run_dm(cmd: DmCmd, api: &Api) -> Result<()> {
    match cmd {
        DmCmd::Send { handle, body } => emit(api.post(
            "/direct-messages",
            Some(json!({ "handle": handle, "body": body })),
        )?),
    }
}
