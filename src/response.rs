use serde::{de, Deserialize, Deserializer, Serialize};
use std::fmt;

use crate::auth::Tokens;

/// A row returned by search / info endpoints. All values are returned as
/// strings (or `null`) by the API, even numeric ones.
pub type Row = serde_json::Map<String, serde_json::Value>;

/// Response of `/search` and `/info` endpoints.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "T: Deserialize<'de>")]
pub struct SearchResponse<T = Row> {
    #[serde(deserialize_with = "de_u64")]
    pub count: u64,
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CountResponse {
    #[serde(deserialize_with = "de_u64")]
    pub count: u64,
}

/// Response of asynchronous `/upload` endpoints.
///
/// The result can be checked by searching the upload queue
/// (`system_que().search(...)` with `que_id-eq`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UploadResponse {
    #[serde(deserialize_with = "de_string")]
    pub que_id: String,
}

/// Response of `/api_neauth` (access_token acquisition).
#[derive(Clone, Deserialize)]
pub struct NeAuthResponse {
    /// Filled from `access_token` / `refresh_token` in the response.
    #[serde(skip)]
    pub tokens: Tokens,
    /// Unique per (purchased app, user).
    pub uid: Option<String>,
    pub company_app_header: Option<String>,
    pub company_ne_id: Option<String>,
    pub company_name: Option<String>,
    pub company_kana: Option<String>,
    /// Unique per user (use this to identify the same person across apps).
    pub pic_ne_id: Option<String>,
    pub pic_name: Option<String>,
    pub pic_kana: Option<String>,
    pub pic_mail_address: Option<String>,
}

impl fmt::Debug for NeAuthResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NeAuthResponse")
            .field("tokens", &self.tokens)
            .field("uid", &self.uid)
            .field("company_ne_id", &self.company_ne_id)
            .field("company_name", &self.company_name)
            .field("pic_ne_id", &self.pic_ne_id)
            .field("pic_name", &self.pic_name)
            .finish_non_exhaustive()
    }
}

/// Accepts `"123"` or `123`.
pub(crate) fn de_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct V;
    impl<'de> de::Visitor<'de> for V {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an integer or a numeric string")
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(E::custom)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.trim().parse().map_err(E::custom)
        }
    }
    d.deserialize_any(V)
}

/// Accepts a string or a number and returns it as a string.
pub(crate) fn de_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(de::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_count() {
        let r: SearchResponse = serde_json::from_str(
            r#"{"result":"success","count":"2","data":[{"a":"1"},{"a":null}]}"#,
        )
        .unwrap();
        assert_eq!(r.count, 2);
        assert_eq!(r.data.len(), 2);
        let u: UploadResponse = serde_json::from_str(r#"{"que_id":189}"#).unwrap();
        assert_eq!(u.que_id, "189");
    }
}
