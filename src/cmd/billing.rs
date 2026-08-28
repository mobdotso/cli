use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, Api};

#[derive(Subcommand)]
pub enum BillingCmd {
    /// Show balances, subscription state, and funding options
    Summary,
    /// List billing history newest first: grants, purchases, charges
    Ledger {
        #[arg(long, default_value_t = 25)]
        limit: u32,
        /// Cursor from an earlier page
        #[arg(long, default_value = "")]
        cursor: String,
    },
    /// Get a Stripe checkout link that saves a payment method
    PaymentMethod,
    /// Get a Stripe checkout link for a monthly subscription
    Subscribe {
        /// Amount in millionths of a US dollar
        #[arg(long)]
        amount_micros: i64,
    },
    /// Get a Stripe checkout link for a one-time top up
    Topup {
        /// Amount in millionths of a US dollar
        #[arg(long)]
        amount_micros: i64,
    },
    /// Get a Stripe billing portal link
    Portal,
}

pub fn run(cmd: BillingCmd, api: &Api) -> Result<()> {
    match cmd {
        BillingCmd::Summary => emit(api.get("/billing/summary")?),
        BillingCmd::Ledger { limit, cursor } => emit(api.get_query(
            "/billing/ledger",
            &[("limit", limit.to_string()), ("cursor", cursor)],
        )?),
        BillingCmd::PaymentMethod => emit(api.post("/billing/payment-method/session", None)?),
        BillingCmd::Subscribe { amount_micros } => emit(api.post(
            "/billing/subscription/session",
            Some(json!({ "amount_micros": amount_micros })),
        )?),
        BillingCmd::Topup { amount_micros } => emit(api.post(
            "/billing/topup/session",
            Some(json!({ "amount_micros": amount_micros })),
        )?),
        BillingCmd::Portal => emit(api.post("/billing/portal/session", None)?),
    }
}
