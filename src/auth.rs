//! Authentication primitives: tokens, pluggable token storage and the
//! sign-in redirect helpers.
//!
//! NextEngine's token model is unusual:
//!
//! * `access_token` (valid 1 day) and `refresh_token` (valid 3 days) are sent
//!   with **every** request.
//! * There is no refresh endpoint. When the access token has expired, the first
//!   request made with a valid refresh token succeeds and the response carries
//!   **new** tokens. The old ones stop working immediately (`002002`).
//! * New tokens can be returned even when `result` is `error`, so they must be
//!   persisted after every response.
//! * The official guide recommends loading tokens from persistent storage
//!   before each request (another process may have rotated them) and not
//!   issuing parallel requests for the same user.
//!
//! This crate models that with the [`TokenStore`] trait: a
//! [`Session`](crate::Session) loads tokens from the store before every
//! request, serializes requests, and saves rotated tokens back. Implement the
//! trait on top of your database / KVS, or use [`MemoryTokenStore`].
//!
//! If you prefer to manage tokens yourself, use
//! [`Client::execute`](crate::Client::execute), which takes tokens explicitly
//! and hands back whatever tokens the API returned.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::error::BoxError;
use crate::response::Row;

/// Boxed `Send` future, used by [`TokenStore`] so that it stays object safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A pair of NextEngine tokens plus their expiry timestamps (as returned by the
/// API, `YYYY-MM-DD HH:MM:SS` in JST).
///
/// `Debug` output redacts the token values.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token_end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token_end_date: Option<String>,
}

impl Tokens {
    pub fn new(access_token: impl Into<String>, refresh_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            access_token_end_date: None,
            refresh_token_end_date: None,
        }
    }

    /// Extract tokens from an API response body, if present.
    pub fn from_response(body: &Row) -> Option<Self> {
        let s = |k: &str| body.get(k).and_then(|v| v.as_str()).map(str::to_owned);
        let access_token = s("access_token").filter(|s| !s.is_empty())?;
        let refresh_token = s("refresh_token").filter(|s| !s.is_empty())?;
        Some(Self {
            access_token,
            refresh_token,
            access_token_end_date: s("access_token_end_date"),
            refresh_token_end_date: s("refresh_token_end_date"),
        })
    }

    /// Whether the token values (ignoring expiry dates) differ from `other`.
    pub fn rotated_from(&self, other: &Tokens) -> bool {
        self.access_token != other.access_token || self.refresh_token != other.refresh_token
    }
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn redact(s: &str) -> String {
            if s.len() <= 8 {
                "***".into()
            } else {
                format!("{}***", &s[..4])
            }
        }
        f.debug_struct("Tokens")
            .field("access_token", &redact(&self.access_token))
            .field("refresh_token", &redact(&self.refresh_token))
            .field("access_token_end_date", &self.access_token_end_date)
            .field("refresh_token_end_date", &self.refresh_token_end_date)
            .finish()
    }
}

/// Persistent storage for one user's tokens (typically one row keyed by `uid`).
///
/// A [`Session`](crate::Session) calls [`load`](Self::load) before every
/// request and [`save`](Self::save) whenever the API returned different
/// tokens (including on API errors).
///
/// ```
/// use ne_api::auth::{BoxFuture, TokenStore, Tokens};
/// use ne_api::BoxError;
///
/// struct DbStore { uid: String /* , pool: PgPool */ }
///
/// impl TokenStore for DbStore {
///     fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
///         Box::pin(async move {
///             // SELECT access_token, refresh_token FROM ne_tokens WHERE uid = $1
///             Ok(None)
///         })
///     }
///     fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
///         Box::pin(async move {
///             // UPDATE ne_tokens SET ... WHERE uid = $1
///             let _ = tokens;
///             Ok(())
///         })
///     }
/// }
/// ```
pub trait TokenStore: Send + Sync {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>>;
    fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>>;
}

impl<S: TokenStore + ?Sized> TokenStore for Arc<S> {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
        (**self).load()
    }
    fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
        (**self).save(tokens)
    }
}

impl<S: TokenStore + ?Sized> TokenStore for Box<S> {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
        (**self).load()
    }
    fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
        (**self).save(tokens)
    }
}

/// In-memory [`TokenStore`]. Cloning shares the same storage, so you can keep
/// a handle to read the latest tokens.
#[derive(Clone, Default)]
pub struct MemoryTokenStore {
    inner: Arc<Mutex<Option<Tokens>>>,
}

impl MemoryTokenStore {
    pub fn new(tokens: Option<Tokens>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(tokens)),
        }
    }

    /// Current tokens.
    pub fn get(&self) -> Option<Tokens> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, tokens: Option<Tokens>) {
        *self.inner.lock().unwrap_or_else(|e| e.into_inner()) = tokens;
    }
}

impl fmt::Debug for MemoryTokenStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MemoryTokenStore")
            .field(&self.get())
            .finish()
    }
}

impl TokenStore for MemoryTokenStore {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
        let t = self.get();
        Box::pin(async move { Ok(t) })
    }
    fn save<'a>(&'a self, tokens: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
        self.set(Some(tokens.clone()));
        Box::pin(async { Ok(()) })
    }
}

/// `uid` and `state` received on the app's redirect URI after sign-in
/// (or when the app is launched from the NextEngine app menu).
///
/// `state` expires after 5 minutes; exchange it with
/// [`Client::neauth`](crate::Client::neauth) right away.
#[derive(Clone, PartialEq, Eq)]
pub struct SignInCallback {
    pub uid: String,
    pub state: String,
}

impl SignInCallback {
    /// Parse a query string such as `uid=...&state=...` (a leading `?` is allowed).
    pub fn from_query(query: &str) -> Option<Self> {
        let query = query.strip_prefix('?').unwrap_or(query);
        let mut uid = None;
        let mut state = None;
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match &*k {
                "uid" => uid = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                _ => {}
            }
        }
        Some(Self {
            uid: uid.filter(|s| !s.is_empty())?,
            state: state.filter(|s| !s.is_empty())?,
        })
    }

    /// Extract from a full redirect URL.
    pub fn from_url(url: &url::Url) -> Option<Self> {
        Self::from_query(url.query()?)
    }
}

impl fmt::Debug for SignInCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignInCallback")
            .field("uid", &self.uid)
            .field("state", &"***")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_callback() {
        let cb = SignInCallback::from_query("?uid=u1&state=s%2B1&foo=bar").unwrap();
        assert_eq!(cb.uid, "u1");
        assert_eq!(cb.state, "s+1");
        assert!(SignInCallback::from_query("uid=u1").is_none());
    }

    #[test]
    fn debug_redacts() {
        let t = Tokens::new("abcdefghijkl", "zyxwvutsrqpo");
        let s = format!("{t:?}");
        assert!(!s.contains("abcdefghijkl"));
        assert!(!s.contains("zyxwvutsrqpo"));
    }
}
