//! **A minimal JSON reader** — enough for `.cvcurve`, `.cvunlock` and a glTF header, and no more.
//!
//! ⚠ **Owned, like the bytecode and the notation parser.** The formats it reads are small and closed,
//! and the reasons for owning it are the ones that apply everywhere else in this project: a dependency
//! that changes its number formatting between versions changes a fingerprint, and a fingerprint that
//! moves without the content moving is a reproduction bug nobody can explain.
//!
//! # Object keys keep their order and duplicates are refused
//!
//! ⚠ **Two rules a general JSON library gets wrong for this use.** Preserving key order keeps a
//! round-trip stable; and a duplicate key in a spec-legal JSON document has no defined winner, so a
//! table with two `"complexity"` rows would silently take one of them. Both would be *fine* for a
//! config file and are not fine for a build input.

use std::collections::BTreeMap;
use std::fmt;

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// ⚠ **One numeric type, and it is `f64`.** The binding contract already forbids `i64` in the
    /// surface because TS numbers are `f64` and integers above 2⁵³ corrupt silently — a reader that
    /// admitted a wider integer here would let a file express something the bindings cannot carry.
    Number(f64),
    Text(String),
    Array(Vec<Json>),
    /// Insertion-ordered.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A member of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// As a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Text(s) => Some(s),
            _ => None,
        }
    }

    /// As a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// As an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// As an object's entries, in file order.
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// The name of this value's shape, for an error message.
    pub fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Number(_) => "a number",
            Json::Text(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }
}

/// Why a document did not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    /// Byte offset.
    pub at: usize,
    /// What went wrong.
    pub what: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "byte {}: {}", self.at, self.what)
    }
}

impl std::error::Error for JsonError {}

/// Read one JSON document.
pub fn parse(src: &str) -> Result<Json, JsonError> {
    let bytes = src.as_bytes();
    let mut at = 0usize;
    skip_space(bytes, &mut at);
    let v = value(bytes, &mut at)?;
    skip_space(bytes, &mut at);
    if at != bytes.len() {
        return Err(err(at, "content after the document"));
    }
    Ok(v)
}

fn err(at: usize, what: &str) -> JsonError {
    JsonError {
        at,
        what: what.to_string(),
    }
}

fn skip_space(b: &[u8], at: &mut usize) {
    while *at < b.len() && matches!(b[*at], b' ' | b'\t' | b'\n' | b'\r') {
        *at += 1;
    }
}

fn value(b: &[u8], at: &mut usize) -> Result<Json, JsonError> {
    skip_space(b, at);
    let Some(&c) = b.get(*at) else {
        return Err(err(*at, "the document ended early"));
    };
    match c {
        b'{' => object(b, at),
        b'[' => array(b, at),
        b'"' => Ok(Json::Text(string(b, at)?)),
        b't' => literal(b, at, "true", Json::Bool(true)),
        b'f' => literal(b, at, "false", Json::Bool(false)),
        b'n' => literal(b, at, "null", Json::Null),
        _ => number(b, at),
    }
}

fn literal(b: &[u8], at: &mut usize, word: &str, v: Json) -> Result<Json, JsonError> {
    if b[*at..].starts_with(word.as_bytes()) {
        *at += word.len();
        Ok(v)
    } else {
        Err(err(*at, "not a JSON value"))
    }
}

fn object(b: &[u8], at: &mut usize) -> Result<Json, JsonError> {
    *at += 1; // '{'
    let mut entries: Vec<(String, Json)> = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    skip_space(b, at);
    if b.get(*at) == Some(&b'}') {
        *at += 1;
        return Ok(Json::Object(entries));
    }
    loop {
        skip_space(b, at);
        if b.get(*at) != Some(&b'"') {
            return Err(err(*at, "an object key must be a string"));
        }
        let key_at = *at;
        let key = string(b, at)?;
        // ⚠ **Refused rather than resolved.** JSON defines no winner for a duplicate key, so a table
        // with two rows of the same name would silently take one of them.
        if seen.insert(key.clone(), ()).is_some() {
            return Err(JsonError {
                at: key_at,
                what: format!("duplicate key {key:?} — JSON defines no winner, so this is refused"),
            });
        }
        skip_space(b, at);
        if b.get(*at) != Some(&b':') {
            return Err(err(*at, "expected ':'"));
        }
        *at += 1;
        entries.push((key, value(b, at)?));
        skip_space(b, at);
        match b.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;
                return Ok(Json::Object(entries));
            }
            _ => return Err(err(*at, "expected ',' or '}'")),
        }
    }
}

fn array(b: &[u8], at: &mut usize) -> Result<Json, JsonError> {
    *at += 1; // '['
    let mut items = Vec::new();
    skip_space(b, at);
    if b.get(*at) == Some(&b']') {
        *at += 1;
        return Ok(Json::Array(items));
    }
    loop {
        items.push(value(b, at)?);
        skip_space(b, at);
        match b.get(*at) {
            Some(b',') => *at += 1,
            Some(b']') => {
                *at += 1;
                return Ok(Json::Array(items));
            }
            _ => return Err(err(*at, "expected ',' or ']'")),
        }
    }
}

fn string(b: &[u8], at: &mut usize) -> Result<String, JsonError> {
    *at += 1; // '"'
    let mut out = String::new();
    loop {
        let Some(&c) = b.get(*at) else {
            return Err(err(*at, "a string never closed"));
        };
        *at += 1;
        match c {
            b'"' => return Ok(out),
            b'\\' => {
                let Some(&e) = b.get(*at) else {
                    return Err(err(*at, "an escape never completed"));
                };
                *at += 1;
                out.push(match e {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'u' => {
                        let hex = b
                            .get(*at..*at + 4)
                            .ok_or_else(|| err(*at, "a \\u escape needs four digits"))?;
                        let code = u32::from_str_radix(
                            std::str::from_utf8(hex).map_err(|_| err(*at, "bad \\u escape"))?,
                            16,
                        )
                        .map_err(|_| err(*at, "bad \\u escape"))?;
                        *at += 4;
                        char::from_u32(code).ok_or_else(|| err(*at, "not a character"))?
                    }
                    _ => return Err(err(*at, "unknown escape")),
                });
            }
            _ => {
                // Multi-byte UTF-8 passes through unchanged.
                let start = *at - 1;
                let len = utf8_len(c);
                let slice = b
                    .get(start..start + len)
                    .ok_or_else(|| err(start, "truncated UTF-8"))?;
                out.push_str(std::str::from_utf8(slice).map_err(|_| err(start, "bad UTF-8"))?);
                *at = start + len;
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn number(b: &[u8], at: &mut usize) -> Result<Json, JsonError> {
    let start = *at;
    if b.get(*at) == Some(&b'-') {
        *at += 1;
    }
    while at_digit(b, *at) || matches!(b.get(*at), Some(b'.' | b'e' | b'E' | b'+' | b'-')) {
        *at += 1;
    }
    if start == *at {
        return Err(err(start, "not a JSON value"));
    }
    let text = std::str::from_utf8(&b[start..*at]).map_err(|_| err(start, "bad number"))?;
    text.parse::<f64>()
        .map(Json::Number)
        .map_err(|_| err(start, "not a number"))
}

fn at_digit(b: &[u8], at: usize) -> bool {
    matches!(b.get(at), Some(c) if c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shapes_a_curve_table_uses_all_read() {
        let v = parse(
            r#"{ "version": 1, "domain": "depth",
                 "rows": { "a": { "interpolation": "CUBIC", "points": [[0.0,1.0],[1.0,6.0]] } } }"#,
        )
        .unwrap();
        assert_eq!(v.get("version").and_then(Json::as_f64), Some(1.0));
        assert_eq!(v.get("domain").and_then(Json::as_str), Some("depth"));
        let points = v
            .get("rows")
            .and_then(|r| r.get("a"))
            .and_then(|a| a.get("points"))
            .and_then(Json::as_array)
            .unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].as_array().unwrap()[1].as_f64(), Some(1.0));
    }

    #[test]
    fn object_keys_keep_their_file_order() {
        // ⚠ So a round-trip is stable rather than alphabetised.
        let v = parse(r#"{"z":1,"a":2,"m":3}"#).unwrap();
        let keys: Vec<&str> = v
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["z", "a", "m"]);
    }

    #[test]
    fn a_duplicate_key_is_refused_rather_than_resolved() {
        // ⚠ JSON defines no winner, so a table with two rows of the same name would silently take one.
        let err = parse(r#"{"complexity":1,"complexity":2}"#).unwrap_err();
        assert!(err.what.contains("duplicate key"));
        assert!(err.what.contains("no winner"));
    }

    #[test]
    fn every_scalar_reads() {
        assert_eq!(parse("null").unwrap(), Json::Null);
        assert_eq!(parse("true").unwrap(), Json::Bool(true));
        assert_eq!(parse("false").unwrap(), Json::Bool(false));
        assert_eq!(parse("-2.5e3").unwrap(), Json::Number(-2500.0));
        assert_eq!(parse(r#""hi""#).unwrap(), Json::Text("hi".into()));
    }

    #[test]
    fn escapes_and_unicode_survive() {
        assert_eq!(
            parse(r#""a\nb\t\"c\\dé""#).unwrap(),
            Json::Text("a\nb\t\"c\\dé".into())
        );
        assert_eq!(
            parse(r#""héllo → ok""#).unwrap().as_str(),
            Some("héllo → ok")
        );
    }

    #[test]
    fn empty_containers_read() {
        assert_eq!(parse("{}").unwrap(), Json::Object(vec![]));
        assert_eq!(parse("[]").unwrap(), Json::Array(vec![]));
        assert_eq!(parse("  [ ]  ").unwrap(), Json::Array(vec![]));
    }

    #[test]
    fn malformed_documents_report_where() {
        for bad in [
            "{",
            "[1,",
            r#"{"a" 1}"#,
            r#"{a:1}"#,
            r#""unterminated"#,
            "{} extra",
            "",
        ] {
            let e = parse(bad).unwrap_err();
            assert!(!e.what.is_empty(), "{bad:?} produced no message");
            assert!(e.to_string().starts_with("byte "), "{bad:?}");
        }
    }

    #[test]
    fn a_nested_document_reads_to_the_right_depth() {
        let v = parse(r#"{"a":{"b":{"c":[1,[2,[3]]]}}}"#).unwrap();
        let inner = v.get("a").unwrap().get("b").unwrap().get("c").unwrap();
        assert_eq!(inner.as_array().unwrap().len(), 2);
    }

    #[test]
    fn kind_names_the_shape_for_an_error_message() {
        assert_eq!(Json::Null.kind(), "null");
        assert_eq!(Json::Number(1.0).kind(), "a number");
        assert_eq!(Json::Array(vec![]).kind(), "an array");
        assert_eq!(Json::Object(vec![]).kind(), "an object");
    }

    #[test]
    fn a_missing_member_is_none_rather_than_a_default() {
        let v = parse(r#"{"a":1}"#).unwrap();
        assert!(v.get("b").is_none());
        assert!(Json::Number(1.0).get("a").is_none());
    }
}
