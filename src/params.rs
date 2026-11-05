use std::fmt;

/// A single form parameter value.
///
/// Values are stored as raw bytes so that binary payloads (e.g. gzip-compressed
/// CSV for `data_type=gz`) can be sent as well as text.
#[derive(Clone, PartialEq, Eq)]
pub struct ParamValue(Vec<u8>);

impl ParamValue {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ParamValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) if s.len() <= 200 => write!(f, "{s:?}"),
            Ok(s) => write!(
                f,
                "{:?}… ({} bytes)",
                &s[..s.floor_char_boundary_compat(200)],
                s.len()
            ),
            Err(_) => write!(f, "<{} bytes>", self.0.len()),
        }
    }
}

trait FloorCharBoundary {
    fn floor_char_boundary_compat(&self, i: usize) -> usize;
}
impl FloorCharBoundary for str {
    fn floor_char_boundary_compat(&self, mut i: usize) -> usize {
        while !self.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

impl From<String> for ParamValue {
    fn from(v: String) -> Self {
        Self(v.into_bytes())
    }
}
impl From<&String> for ParamValue {
    fn from(v: &String) -> Self {
        Self(v.as_bytes().to_vec())
    }
}
impl From<&str> for ParamValue {
    fn from(v: &str) -> Self {
        Self(v.as_bytes().to_vec())
    }
}
impl From<Vec<u8>> for ParamValue {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}
impl From<&[u8]> for ParamValue {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}
macro_rules! from_display {
    ($($t:ty),*) => {$(
        impl From<$t> for ParamValue {
            fn from(v: $t) -> Self { Self(v.to_string().into_bytes()) }
        }
    )*};
}
from_display!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

/// Ordered set of POST form parameters. Setting an existing key replaces it
/// (NextEngine does not accept the same key twice).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params {
    entries: Vec<(String, ParamValue)>,
}

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style [`insert`](Self::insert).
    pub fn set(mut self, key: impl Into<String>, value: impl Into<ParamValue>) -> Self {
        self.insert(key, value);
        self
    }

    /// Insert or replace a parameter.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<ParamValue>) {
        let key = key.into();
        let value = value.into();
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<ParamValue> {
        let pos = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(pos).1)
    }

    pub fn get(&self, key: &str) -> Option<&ParamValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &ParamValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Merge `other` into `self` (values in `other` win).
    pub fn extend(&mut self, other: Params) {
        for (k, v) in other.entries {
            self.insert(k, v);
        }
    }

    /// `application/x-www-form-urlencoded` body.
    pub(crate) fn encode(&self) -> String {
        let mut out = String::new();
        for (i, (k, v)) in self.entries.iter().enumerate() {
            if i > 0 {
                out.push('&');
            }
            out.extend(url::form_urlencoded::byte_serialize(k.as_bytes()));
            out.push('=');
            out.extend(url::form_urlencoded::byte_serialize(v.as_bytes()));
        }
        out
    }
}

impl<K: Into<String>, V: Into<ParamValue>> FromIterator<(K, V)> for Params {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut p = Params::new();
        for (k, v) in iter {
            p.insert(k, v);
        }
        p
    }
}

impl<K: Into<String>, V: Into<ParamValue>, const N: usize> From<[(K, V); N]> for Params {
    fn from(arr: [(K, V); N]) -> Self {
        arr.into_iter().collect()
    }
}

impl<K: Into<String>, V: Into<ParamValue>> From<Vec<(K, V)>> for Params {
    fn from(v: Vec<(K, V)>) -> Self {
        v.into_iter().collect()
    }
}

impl From<&Params> for Params {
    fn from(p: &Params) -> Self {
        p.clone()
    }
}

impl From<()> for Params {
    fn from(_: ()) -> Self {
        Params::new()
    }
}

/// Join values into a NextEngine comma-separated list.
///
/// Values containing a comma, double quote or newline are wrapped in double
/// quotes and inner quotes are doubled, as required by the API
/// ("カンマ区切りの指定について").
pub fn csv_list<I, S>(values: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = String::new();
    for (i, v) in values.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let v = v.as_ref();
        if v.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&v.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quoting() {
        assert_eq!(csv_list(["a", "b"]), "a,b");
        assert_eq!(csv_list(["a,b", "c\"d"]), "\"a,b\",\"c\"\"d\"");
    }

    #[test]
    fn insert_replaces() {
        let p = Params::new().set("a", "1").set("b", "2").set("a", "3");
        assert_eq!(p.encode(), "a=3&b=2");
    }

    #[test]
    fn encodes_binary_and_japanese() {
        let p = Params::new().set("x", "受注 %").set("d", vec![0u8, 255]);
        assert_eq!(p.encode(), "x=%E5%8F%97%E6%B3%A8+%25&d=%00%FF");
    }
}
