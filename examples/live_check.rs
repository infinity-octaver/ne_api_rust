//! End-to-end check against the real API.
//!
//! 1. Print the sign-in URL:
//!    `NE_CLIENT_ID=.. NE_CLIENT_SECRET=.. NE_REDIRECT_URI=.. cargo run --example live_check -- url`
//! 2. Open it in a browser, sign in, and copy `uid` / `state` from the redirect URL.
//! 3. Exchange them for tokens (saved to `NE_TOKENS_FILE`) and call a few endpoints:
//!    `cargo run --example live_check -- auth '<redirect url or query>'`
//! 4. Later runs reuse (and keep rotating) the saved tokens:
//!    `cargo run --example live_check -- check`
//!
//! Credentials are read from the environment only.

use std::path::PathBuf;

use ne_api::auth::BoxFuture;
use ne_api::{BoxError, Client, SearchQuery, SignInCallback, TokenStore, Tokens};

/// Example [`TokenStore`] that keeps tokens in a JSON file.
struct FileTokenStore(PathBuf);

impl TokenStore for FileTokenStore {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
        Box::pin(async move {
            match std::fs::read(&self.0) {
                Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
        Box::pin(async move {
            std::fs::write(&self.0, serde_json::to_vec_pretty(tokens)?)?;
            eprintln!(
                "[token store] saved tokens (access expires {:?})",
                tokens.access_token_end_date
            );
            Ok(())
        })
    }
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} is not set"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(env("NE_CLIENT_ID"), env("NE_CLIENT_SECRET"))?;
    let store = FileTokenStore(env("NE_TOKENS_FILE").into());
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("url") => {
            let redirect = std::env::var("NE_REDIRECT_URI").ok();
            println!("{}", client.sign_in_url(redirect.as_deref(), ()));
            return Ok(());
        }
        Some("auth") => {
            let input = args
                .get(1)
                .expect("pass the redirect URL or its query string");
            let callback = match url::Url::parse(input) {
                Ok(u) => SignInCallback::from_url(&u),
                Err(_) => SignInCallback::from_query(input),
            }
            .expect("uid/state not found");
            let auth = client.neauth_callback(&callback).await?;
            println!(
                "signed in: company={:?} user={:?}",
                auth.company_name, auth.pic_name
            );
            store.save(&auth.tokens).await.map_err(|e| e.to_string())?;
        }
        Some("check") => {}
        _ => {
            eprintln!("usage: live_check url | auth <redirect-url> | check");
            std::process::exit(2);
        }
    }

    let session = client.session(store);

    let company = session.login_company().info().await?;
    println!(
        "login_company/info: {:?}",
        company.data.first().and_then(|r| r.get("company_name"))
    );

    let statuses = session.system_orderstatus().info().await?;
    println!("system_orderstatus/info: {} statuses", statuses.count);

    let shops = session
        .master_shop()
        .search(
            &SearchQuery::new()
                .fields(["shop_id", "shop_name", "shop_mall_id"])
                .limit(5),
        )
        .await?;
    println!("master_shop/search: {} shops", shops.count);
    for s in &shops.data {
        println!("  {:?} {:?}", s.get("shop_id"), s.get("shop_name"));
    }

    let orders = session
        .receiveorder_base()
        .count(&SearchQuery::new().eq("receive_order_cancel_type_id", "0"))
        .await?;
    println!("receiveorder_base/count (not cancelled): {orders}");

    Ok(())
}
