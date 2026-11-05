use crate::params::{csv_list, ParamValue, Params};

/// Comparison operators for search / count conditions (`<field>-<op>=<value>`).
///
/// See <https://developer.next-engine.com/guides/param/operator_compare/>.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Operator {
    /// `=`
    Eq,
    /// `!=` (drops NULL rows)
    Neq,
    /// `<` (numeric / datetime only)
    Lt,
    /// `<=` (numeric / datetime only)
    Lte,
    /// `>` (numeric / datetime only)
    Gt,
    /// `>=` (numeric / datetime only)
    Gte,
    /// `IN (...)`, up to 1000 values
    In,
    /// `NOT IN (...)`, up to 1000 values (drops NULL rows)
    Nin,
    /// MySQL `LIKE`
    Like,
    /// MySQL `NOT LIKE` (drops NULL rows)
    Nlike,
    /// `NOT LIKE ... OR IS NULL`
    NlikeOrNull,
    /// `IS NULL OR = ""`
    Blank,
    /// `IS NOT NULL AND <> ""`
    Nblank,
    /// `IS NULL`
    Null,
    /// `IS NOT NULL`
    Nnull,
}

impl Operator {
    /// The suffix appended to the field name, e.g. `"-eq"`.
    pub fn suffix(self) -> &'static str {
        match self {
            Operator::Eq => "-eq",
            Operator::Neq => "-neq",
            Operator::Lt => "-lt",
            Operator::Lte => "-lte",
            Operator::Gt => "-gt",
            Operator::Gte => "-gte",
            Operator::In => "-in",
            Operator::Nin => "-nin",
            Operator::Like => "-like",
            Operator::Nlike => "-nlike",
            Operator::NlikeOrNull => "-nlikeornull",
            Operator::Blank => "-blank",
            Operator::Nblank => "-nblank",
            Operator::Null => "-null",
            Operator::Nnull => "-nnull",
        }
    }
}

/// Builder for search / count requests.
///
/// All conditions are combined with AND (the API has no OR). Setting the same
/// field + operator twice replaces the previous value.
///
/// ```
/// use ne_api::SearchQuery;
///
/// let q = SearchQuery::new()
///     .fields(["receive_order_id", "receive_order_date"])
///     .in_("receive_order_order_status_id", ["0", "1", "2", "20"])
///     .eq("receive_order_cancel_type_id", "0")
///     .gte("receive_order_date", "2026-11-01 00:00:00")
///     .limit(500);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    fields: Vec<String>,
    conditions: Params,
    offset: Option<u64>,
    limit: Option<u64>,
    extra: Params,
}

impl SearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fields to return (`fields`, required for search). Replaces previously set fields.
    pub fn fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fields = fields.into_iter().map(Into::into).collect();
        self
    }

    /// Add one field to return.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Add a condition with an arbitrary operator.
    pub fn cond(mut self, field: &str, op: Operator, value: impl Into<ParamValue>) -> Self {
        self.conditions
            .insert(format!("{field}{}", op.suffix()), value);
        self
    }

    pub fn eq(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Eq, value)
    }
    pub fn neq(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Neq, value)
    }
    pub fn lt(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Lt, value)
    }
    pub fn lte(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Lte, value)
    }
    pub fn gt(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Gt, value)
    }
    pub fn gte(self, field: &str, value: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Gte, value)
    }
    pub fn like(self, field: &str, pattern: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Like, pattern)
    }
    pub fn nlike(self, field: &str, pattern: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::Nlike, pattern)
    }
    pub fn nlike_or_null(self, field: &str, pattern: impl Into<ParamValue>) -> Self {
        self.cond(field, Operator::NlikeOrNull, pattern)
    }

    /// `-in` with proper CSV quoting. Max 1000 values.
    pub fn in_<I, S>(self, field: &str, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.cond(field, Operator::In, csv_list(values))
    }

    /// `-nin` with proper CSV quoting. Max 1000 values.
    pub fn nin<I, S>(self, field: &str, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.cond(field, Operator::Nin, csv_list(values))
    }

    pub fn blank(self, field: &str) -> Self {
        self.cond(field, Operator::Blank, "")
    }
    pub fn nblank(self, field: &str) -> Self {
        self.cond(field, Operator::Nblank, "")
    }
    pub fn is_null(self, field: &str) -> Self {
        self.cond(field, Operator::Null, "")
    }
    pub fn not_null(self, field: &str) -> Self {
        self.cond(field, Operator::Nnull, "")
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Max rows to return (API default: 10000).
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Any other parameter (e.g. `receive_order_ids` for tax account search, `wait_flag`).
    pub fn param(mut self, key: impl Into<String>, value: impl Into<ParamValue>) -> Self {
        self.extra.insert(key, value);
        self
    }

    pub fn get_offset(&self) -> Option<u64> {
        self.offset
    }
    pub fn get_limit(&self) -> Option<u64> {
        self.limit
    }

    /// Parameters for a `/search` request.
    pub fn to_search_params(&self) -> Params {
        let mut p = Params::new();
        if !self.fields.is_empty() {
            p.insert("fields", self.fields.join(","));
        }
        p.extend(self.conditions.clone());
        if let Some(o) = self.offset {
            p.insert("offset", o);
        }
        if let Some(l) = self.limit {
            p.insert("limit", l);
        }
        p.extend(self.extra.clone());
        p
    }

    /// Parameters for a `/count` request (conditions only; fields / paging are omitted).
    pub fn to_count_params(&self) -> Params {
        let mut p = self.conditions.clone();
        p.extend(self.extra.clone());
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_params() {
        let q = SearchQuery::new()
            .fields(["a", "b"])
            .in_("s", ["1", "2"])
            .eq("c", "0")
            .eq("c", "1")
            .blank("x")
            .limit(10);
        assert_eq!(
            q.to_search_params().encode(),
            "fields=a%2Cb&s-in=1%2C2&c-eq=1&x-blank=&limit=10"
        );
        assert_eq!(q.to_count_params().encode(), "s-in=1%2C2&c-eq=1&x-blank=");
    }
}
