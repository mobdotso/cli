use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};

#[derive(Subcommand)]
pub enum ServiceKeysCmd {
    /// List this account's service keys
    List,
    /// Issue a service key. The plaintext prints once
    Create {
        #[arg(long, default_value = "")]
        name: String,
    },
    /// Revoke a service key
    Revoke { key_id: String },
}

pub fn run(cmd: ServiceKeysCmd, api: &Api) -> Result<()> {
    match cmd {
        ServiceKeysCmd::List => emit(api.get("/service-keys")?),
        ServiceKeysCmd::Create { name } => {
            emit(api.post("/service-keys", Some(json!({ "name": name })))?)
        }
        ServiceKeysCmd::Revoke { key_id } => {
            emit(api.delete(&format!("/service-keys/{}", seg(&key_id)))?)
        }
    }
}
