use reqwest::StatusCode;

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Boxed error returned by user-supplied [`TokenStore`](crate::TokenStore) implementations.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Message codes that the official documentation marks as recoverable
/// ("復帰: ○"), i.e. retrying later with the same parameters can succeed.
///
/// `003002` (main function overloaded) is also included: it is returned when
/// `wait_flag` is not set and the server is busy.
const RETRYABLE_CODES: &[&str] = &[
    "002007", "003001", "003002", "003004", "003005", "008003", "008007", "008010", "008011",
    "009005", "009006", "010003", "010009", "011007", "011011", "016011", "016023", "020007",
    "020011", "024003", "024007", "024010", "024011",
];

/// Errors returned by the client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API returned `"result": "error"`.
    #[error("NextEngine API error [{code}]: {message}")]
    Api { code: String, message: String },

    /// The API returned `"result": "redirect"`. The user has to sign in again
    /// (uid/state → access_token) before the API can be used.
    #[error("NextEngine API requires re-authentication [{code}]: {message}")]
    Redirect { code: String, message: String },

    /// A non-success HTTP status without a parsable API body.
    #[error("HTTP {status}: {body}")]
    Http { status: StatusCode, body: String },

    /// Network / TLS / timeout error.
    #[error(transparent)]
    Transport(#[from] reqwest::Error),

    /// The response body was not valid JSON.
    #[error("invalid JSON response: {source}")]
    InvalidJson {
        #[source]
        source: serde_json::Error,
        body: String,
    },

    /// The JSON response could not be converted into the requested type.
    #[error("failed to deserialize response: {0}")]
    Deserialize(#[source] serde_json::Error),

    /// The response did not look like a NextEngine API response.
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),

    /// The token store has no tokens for this session.
    #[error("no tokens available; run the sign-in flow (uid/state → neauth) first")]
    NotAuthenticated,

    /// The [`TokenStore`](crate::TokenStore) failed to load or save tokens.
    #[error("token store error: {0}")]
    TokenStore(#[source] BoxError),

    /// An invalid base URL was configured.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),
}

impl Error {
    /// The NextEngine message code (e.g. `"002004"`), if this is an API error.
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api { code, .. } | Error::Redirect { code, .. } => Some(code),
            _ => None,
        }
    }

    /// Whether retrying the same request later may succeed
    /// (maintenance, overload, order import in progress, …).
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Api { code, .. } => RETRYABLE_CODES.contains(&code.as_str()),
            Error::Http { status, .. } => status.is_server_error(),
            Error::Transport(e) => e.is_timeout() || e.is_connect(),
            _ => false,
        }
    }

    /// Whether the user has to sign in again to obtain new tokens
    /// (`result=redirect`, missing tokens, or invalid / expired tokens: `002002`, `002004`).
    pub fn requires_reauth(&self) -> bool {
        match self {
            Error::Redirect { .. } | Error::NotAuthenticated => true,
            Error::Api { code, .. } => code == "002002" || code == "002004",
            _ => false,
        }
    }
}
