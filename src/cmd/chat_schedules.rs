use anyhow::Result;
use clap::Subcommand;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum ChatSchedulesCmd {
    /// List recurring tasks for the main Chat assistant
    List {
        /// next_cursor from the previous response
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Read one recurring task
    Get { schedule_id: String },
    /// Read a recurring task's run history
    Runs {
        schedule_id: String,
        /// next_cursor from the previous response
        #[arg(long)]
        cursor: Option<String>,
    },
}

pub fn run(cmd: ChatSchedulesCmd, api: &Api) -> Result<()> {
    let (path, cursor) = match cmd {
        ChatSchedulesCmd::List { cursor } => ("/chat-schedules".to_owned(), cursor),
        ChatSchedulesCmd::Get { schedule_id } => {
            (format!("/chat-schedules/{}", seg(&schedule_id)), None)
        }
        ChatSchedulesCmd::Runs {
            schedule_id,
            cursor,
        } => (
            format!("/chat-schedules/{}/runs", seg(&schedule_id)),
            cursor,
        ),
    };
    let query: Vec<_> = cursor.into_iter().map(|value| ("cursor", value)).collect();
    emit(api.get_query(&path, &query)?)
}
