use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use url::Url;

use crate::auth::{SignInCallback, TokenStore, Tokens};
use crate::error::{Error, Result};
use crate::params::Params;
use crate::response::{NeAuthResponse, Row, SearchResponse};
use crate::session::Session;

/// Default API host.
pub const DEFAULT_API_BASE_URL: &str = "https://api.next-engine.org";
/// Default host for the sign-in (uid/state) redirect.
pub const DEFAULT_SIGN_IN_BASE_URL: &str = "https://base.next-engine.org";

/// The result of one API call made with explicit tokens.
///
/// `tokens` holds the tokens the API returned in this response, if any. They
/// may differ from the ones sent (rotation after expiry) and may be present
/// even when `result` is an error. Persist them whenever they change.
#[derive(Debug)]
pub struct Exchange<T> {
    pub tokens: Option<Tokens>,
    pub result: Result<T>,
}

impl<T> Exchange<T> {
    /// Map the success value.
    pub fn map<U>(self, f: impl FnOnce(T) -> Result<U>) -> Exchange<U> {
        Exchange {
            tokens: self.tokens,
            result: self.result.and_then(f),
        }
    }
}

/// Application-level client: holds `client_id` / `client_secret`, the HTTP
/// connection pool and configuration, but **no user tokens**.
///
/// It is cheap to clone and should be shared across users. Create a
/// [`Session`] per user to call the API with automatic token handling, or use
/// [`execute`](Self::execute) to manage tokens yourself.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    api_base: String,
    sign_in_base: Url,
    wait_flag: bool,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("client_id", &self.inner.client_id)
            .field("api_base", &self.inner.api_base)
            .field("wait_flag", &self.inner.wait_flag)
            .finish_non_exhaustive()
    }
}

/// Builder for [`Client`].
pub struct ClientBuilder {
    client_id: String,
    client_secret: String,
    api_base: String,
    sign_in_base: String,
    wait_flag: bool,
    http: Option<reqwest::Client>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Override the API base URL (e.g. for a mock server).
    pub fn api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base = url.into();
        self
    }

    /// Override the sign-in base URL.
    pub fn sign_in_base_url(mut self, url: impl Into<String>) -> Self {
        self.sign_in_base = url.into();
        self
    }

    /// Send `wait_flag=1` on every token-authenticated request (default: `false`).
    ///
    /// With it the API waits (up to 60 minutes) instead of failing with
    /// `003002` when the main function is overloaded. Recommended for batch
    /// jobs, not for interactive requests. Can be overridden per request by
    /// passing `wait_flag` in the params.
    pub fn wait_flag(mut self, enabled: bool) -> Self {
        self.wait_flag = enabled;
        self
    }

    /// Use a preconfigured `reqwest::Client` (proxies, custom TLS, …).
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Request timeout for the default HTTP client. Ignored if
    /// [`http_client`](Self::http_client) is set. Note that with `wait_flag`
    /// the server may legitimately take up to an hour.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<Client> {
        Url::parse(&self.api_base)?;
        let sign_in_base = Url::parse(&self.sign_in_base)?;
        let http = match self.http {
            Some(h) => h,
            None => {
                let mut b = reqwest::Client::builder()
                    .user_agent(concat!("ne_api-rust/", env!("CARGO_PKG_VERSION")));
                if let Some(t) = self.timeout {
                    b = b.timeout(t);
                }
                b.build()?
            }
        };
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                client_id: self.client_id,
                client_secret: self.client_secret,
                api_base: self.api_base.trim_end_matches('/').to_owned(),
                sign_in_base,
                wait_flag: self.wait_flag,
            }),
        })
    }
}

impl Client {
    pub fn builder(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> ClientBuilder {
        ClientBuilder {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            api_base: DEFAULT_API_BASE_URL.into(),
            sign_in_base: DEFAULT_SIGN_IN_BASE_URL.into(),
            wait_flag: false,
            http: None,
            timeout: None,
        }
    }

    /// Client with default settings.
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Result<Self> {
        Self::builder(client_id, client_secret).build()
    }

    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    // ---------------------------------------------------------------- sessions

    /// A per-user session backed by `store`.
    pub fn session(&self, store: impl TokenStore + 'static) -> Session {
        Session::new(self.clone(), store)
    }

    /// A per-user session backed by an in-memory store seeded with `tokens`.
    pub fn session_with_tokens(&self, tokens: Tokens) -> Session {
        Session::with_tokens(self.clone(), tokens)
    }

    // ------------------------------------------------------------ sign-in flow

    /// URL to send the user's browser to (NEログインuid・state取得).
    ///
    /// After sign-in NextEngine redirects to `redirect_uri` (or the Redirect
    /// URI registered for the app) with `uid` and `state` appended; `extra`
    /// parameters are passed through unchanged. Parse them with
    /// [`SignInCallback`] and call [`neauth`](Self::neauth).
    pub fn sign_in_url(&self, redirect_uri: Option<&str>, extra: impl Into<Params>) -> Url {
        let mut url = self.inner.sign_in_base.clone();
        url.set_path("/users/sign_in/");
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("client_id", &self.inner.client_id);
            if let Some(r) = redirect_uri {
                q.append_pair("redirect_uri", r);
            }
            for (k, v) in extra.into().iter() {
                q.append_pair(k, &String::from_utf8_lossy(v.as_bytes()));
            }
        }
        url
    }

    /// Exchange `uid` / `state` for tokens (NEログインaccess_token取得, `/api_neauth`).
    ///
    /// Persist `response.tokens` (e.g. via [`TokenStore::save`]) keyed by
    /// `response.uid`.
    pub async fn neauth(&self, uid: &str, state: &str) -> Result<NeAuthResponse> {
        let params = Params::new()
            .set("uid", uid)
            .set("state", state)
            .set("client_id", self.inner.client_id.as_str())
            .set("client_secret", self.inner.client_secret.as_str());
        let (_, result) = self.post("/api_neauth", params).await;
        let body = result?;
        let tokens = Tokens::from_response(&body)
            .ok_or_else(|| Error::UnexpectedResponse("neauth response has no tokens".into()))?;
        let mut res: NeAuthResponse =
            serde_json::from_value(serde_json::Value::Object(body)).map_err(Error::Deserialize)?;
        res.tokens = tokens;
        Ok(res)
    }

    /// [`neauth`](Self::neauth) taking a parsed redirect callback.
    pub async fn neauth_callback(&self, callback: &SignInCallback) -> Result<NeAuthResponse> {
        self.neauth(&callback.uid, &callback.state).await
    }

    /// Companies currently allowed to use the app (アプリ利用企業一覧, `/api_app/company`).
    /// Authenticated with `client_id` / `client_secret`, no user tokens needed.
    pub async fn app_companies(&self, params: impl Into<Params>) -> Result<SearchResponse<Row>> {
        let mut p = params.into();
        p.insert("client_id", self.inner.client_id.as_str());
        p.insert("client_secret", self.inner.client_secret.as_str());
        let (_, result) = self.post("/api_app/company", p).await;
        serde_json::from_value(serde_json::Value::Object(result?)).map_err(Error::Deserialize)
    }

    // -------------------------------------------------------- explicit tokens

    /// Call `path` with explicit tokens and return the raw JSON object along
    /// with the tokens contained in the response.
    ///
    /// This is the low-level building block for callers who manage tokens
    /// themselves; [`Session`] is built on top of it.
    pub async fn execute(
        &self,
        tokens: &Tokens,
        path: &str,
        params: impl Into<Params>,
    ) -> Exchange<Row> {
        let mut p = Params::new()
            .set("access_token", tokens.access_token.as_str())
            .set("refresh_token", tokens.refresh_token.as_str());
        if self.inner.wait_flag {
            p.insert("wait_flag", "1");
        }
        p.extend(params.into());
        let (tokens, result) = self.post(path, p).await;
        Exchange { tokens, result }
    }

    /// [`execute`](Self::execute) with the body deserialized into `T`.
    pub async fn execute_as<T: DeserializeOwned>(
        &self,
        tokens: &Tokens,
        path: &str,
        params: impl Into<Params>,
    ) -> Exchange<T> {
        self.execute(tokens, path, params).await.map(|row| {
            serde_json::from_value(serde_json::Value::Object(row)).map_err(Error::Deserialize)
        })
    }

    // --------------------------------------------------------------- transport

    async fn post(&self, path: &str, params: Params) -> (Option<Tokens>, Result<Row>) {
        let url = format!("{}{}", self.inner.api_base, path);
        let resp = match self
            .inner
            .http
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(params.encode())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return (None, Err(e.into())),
        };
        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => return (None, Err(e.into())),
        };
        parse_body(status, body)
    }
}

fn parse_body(status: reqwest::StatusCode, body: String) -> (Option<Tokens>, Result<Row>) {
    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(source) => {
            let err = if status.is_success() {
                Error::InvalidJson { source, body }
            } else {
                Error::Http { status, body }
            };
            return (None, Err(err));
        }
    };
    let serde_json::Value::Object(map) = value else {
        return (None, Err(Error::UnexpectedResponse(body)));
    };
    let tokens = Tokens::from_response(&map);

    let text = |k: &str| match map.get(k) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    };
    let result = match map.get("result").and_then(|v| v.as_str()) {
        Some("success") => Ok(map),
        Some("error") => Err(Error::Api {
            code: text("code"),
            message: text("message"),
        }),
        Some("redirect") => Err(Error::Redirect {
            code: text("code"),
            message: text("message"),
        }),
        _ if !status.is_success() => Err(Error::Http { status, body }),
        _ => Err(Error::UnexpectedResponse(body)),
    };
    (tokens, result)
}
