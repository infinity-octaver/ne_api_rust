use std::fmt;
use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::auth::{MemoryTokenStore, TokenStore, Tokens};
use crate::client::Client;
use crate::error::{Error, Result};
use crate::params::Params;
use crate::query::SearchQuery;
use crate::response::{CountResponse, Row, SearchResponse, UploadResponse};

/// Default page size used by `search_all` when the query has no `limit`.
pub const DEFAULT_PAGE_SIZE: u64 = 1000;

/// A user's API session: a [`Client`] plus a [`TokenStore`].
///
/// For every request the session
///
/// 1. loads the current tokens from the store,
/// 2. sends them with the request,
/// 3. saves the tokens returned by the API if they changed — also when the
///    API returned an error, as the documentation requires.
///
/// Requests made through one `Session` (and its clones) are serialized, since
/// concurrent requests for the same user can race on token rotation. Requests
/// from other processes are covered by reloading from the store each time.
///
/// Endpoint groups are available as methods, e.g.
/// `session.receiveorder_base().search(&query)`.
#[derive(Clone)]
pub struct Session {
    client: Client,
    store: Arc<dyn TokenStore>,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl Session {
    pub fn new(client: Client, store: impl TokenStore + 'static) -> Self {
        Self {
            client,
            store: Arc::new(store),
            lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Session backed by a [`MemoryTokenStore`] seeded with `tokens`.
    /// Read the latest tokens back with [`tokens`](Self::tokens).
    pub fn with_tokens(client: Client, tokens: Tokens) -> Self {
        Self::new(client, MemoryTokenStore::new(Some(tokens)))
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn store(&self) -> &Arc<dyn TokenStore> {
        &self.store
    }

    /// Current tokens from the store.
    pub async fn tokens(&self) -> Result<Option<Tokens>> {
        self.store.load().await.map_err(Error::TokenStore)
    }

    /// POST to any endpoint path (e.g. `"/api_v1_receiveorder_base/search"`)
    /// and return the raw JSON object.
    pub async fn request_raw(&self, path: &str, params: impl Into<Params>) -> Result<Row> {
        let params = params.into();
        let _guard = self.lock.lock().await;
        let current = self
            .store
            .load()
            .await
            .map_err(Error::TokenStore)?
            .ok_or(Error::NotAuthenticated)?;
        let exchange = self.client.execute(&current, path, params).await;
        if let Some(new) = &exchange.tokens {
            if *new != current {
                self.store.save(new).await.map_err(Error::TokenStore)?;
            }
        }
        exchange.result
    }

    /// [`request_raw`](Self::request_raw) deserialized into `T`.
    pub async fn request<T: DeserializeOwned>(
        &self,
        path: &str,
        params: impl Into<Params>,
    ) -> Result<T> {
        let row = self.request_raw(path, params).await?;
        serde_json::from_value(serde_json::Value::Object(row)).map_err(Error::Deserialize)
    }

    /// `<base>/count`
    pub async fn count(&self, path: &str, query: &SearchQuery) -> Result<u64> {
        Ok(self
            .request::<CountResponse>(path, query.to_count_params())
            .await?
            .count)
    }

    /// `<base>/search` with rows as `T`.
    pub async fn search_as<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &SearchQuery,
    ) -> Result<SearchResponse<T>> {
        self.request(path, query.to_search_params()).await
    }

    /// Fetch all pages of a search by advancing `offset`.
    ///
    /// The page size is the query's `limit` (default [`DEFAULT_PAGE_SIZE`]).
    /// Rows changed between page requests may be skipped or duplicated; sort
    /// stability is not guaranteed by the API, so prefer narrow conditions
    /// (e.g. a date range) for large data sets.
    pub async fn search_all_as<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &SearchQuery,
    ) -> Result<Vec<T>> {
        let page = query.get_limit().unwrap_or(DEFAULT_PAGE_SIZE).max(1);
        let mut offset = query.get_offset().unwrap_or(0);
        let mut all = Vec::new();
        loop {
            let q = query.clone().offset(offset).limit(page);
            let res: SearchResponse<T> = self.search_as(path, &q).await?;
            let n = res.data.len() as u64;
            all.extend(res.data);
            if n < page {
                return Ok(all);
            }
            offset += n;
        }
    }

    pub(crate) async fn upload_at(
        &self,
        path: &str,
        params: impl Into<Params>,
    ) -> Result<UploadResponse> {
        self.request(path, params).await
    }
}
