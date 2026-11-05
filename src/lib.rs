//! Async Rust client for the [NextEngine API](https://developer.next-engine.com/).
//!
//! # Overview
//!
//! * [`Client`] — app-wide: `client_id` / `client_secret`, HTTP pool, sign-in
//!   helpers ([`Client::sign_in_url`], [`Client::neauth`]) and the low-level
//!   [`Client::execute`] that takes tokens explicitly.
//! * [`TokenStore`] — where a user's tokens live. Implement it for your DB.
//! * [`Session`] — one user's session (`Client` + `TokenStore`); loads tokens
//!   before each request, saves rotated tokens after each response, and
//!   exposes every endpoint (`session.receiveorder_base().search(..)`).
//!
//! ```no_run
//! use ne_api::{Client, SearchQuery, Tokens};
//!
//! # async fn run() -> ne_api::Result<()> {
//! let client = Client::new("CLIENT_ID", "CLIENT_SECRET")?;
//! let session = client.session_with_tokens(Tokens::new("ACCESS", "REFRESH"));
//!
//! let orders = session
//!     .receiveorder_base()
//!     .search(
//!         &SearchQuery::new()
//!             .fields(["receive_order_id", "receive_order_date", "receive_order_total_amount"])
//!             .eq("receive_order_cancel_type_id", "0")
//!             .gte("receive_order_date", "2026-11-01 00:00:00")
//!             .limit(100),
//!     )
//!     .await?;
//! println!("{} orders", orders.count);
//!
//! // Persist rotated tokens.
//! let latest = session.tokens().await?;
//! # let _ = latest;
//! # Ok(()) }
//! ```

pub mod auth;
mod client;
pub mod endpoints;
mod error;
mod params;
mod query;
mod response;
mod session;

pub use auth::{MemoryTokenStore, SignInCallback, TokenStore, Tokens};
pub use client::{Client, ClientBuilder, Exchange, DEFAULT_API_BASE_URL, DEFAULT_SIGN_IN_BASE_URL};
pub use error::{BoxError, Error, Result};
pub use params::{csv_list, ParamValue, Params};
pub use query::{Operator, SearchQuery};
pub use response::{NeAuthResponse, Row, SearchResponse, UploadResponse};
pub use session::{Session, DEFAULT_PAGE_SIZE};
