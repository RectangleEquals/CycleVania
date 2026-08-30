//! A **strict subset** of TOML — exactly the constructs `manifest/tier1.toml` uses, and nothing else.
//!
//! Accepted:
//!
//! ```text
//! # comment
//! version = 1
//! [[class]]
//! key = "string" | true | false | 42
//! args = [ { name = "ctx", type = "Ref<Context>" }, … ]
//!   [[class.field]] / [[class.method]] / [[class.value]]
//! ```
//!
//! Everything else is an error with a line number. That is the point: a general TOML implementation
//! would happily accept a nested table, a date, or a float where the schema expects none, and turn a
//! typo into a silently-wrong manifest that generates a silently-wrong palette.

use crate::model::{Class, EnumValue, Field, Kind, Manifest, Method, Param, Status, Value};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "manifest line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        line,
        message: message.into(),
    })
}

/// Which `[[table]]` we are currently filling.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    None,
    Class,
    Field,
    Method,
    EnumValue,
}

/// Parse the manifest text.
pub fn parse(src: &str) -> Result<Manifest, ParseError> {
    let mut m = Manifest::default();
    let mut section = Section::None;

    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }

        if let Some(header) = t.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            section = match header.trim() {
                "class" => {
                    m.classes.push(Class::default());
                    Section::Class
                }
                "class.field" => {
                    open_class(&mut m, line, "field")?
                        .fields
                        .push(Field::default());
                    Section::Field
                }
                "class.method" => {
                    open_class(&mut m, line, "method")?
                        .methods
                        .push(Method::default());
                    Section::Method
                }
                "class.value" => {
                    open_class(&mut m, line, "value")?
                        .values
                        .push(EnumValue::default());
                    Section::EnumValue
                }
                other => return err(line, format!("unknown table `[[{other}]]`")),
            };
            continue;
        }

        if t.starts_with('[') {
            return err(line, "single-bracket tables are not part of the schema");
        }

        let (key, rest) = match t.split_once('=') {
            Some(p) => (p.0.trim(), p.1.trim()),
            None => return err(line, "expected `key = value`"),
        };
        let value = parse_value(line, rest)?;

        match section {
            Section::None => {
                if key == "version" {
                    m.version = as_int(line, key, &value)?;
                } else {
                    return err(line, format!("`{key}` appears before any [[class]]"));
                }
            }
            Section::Class => {
                let c = m.classes.last_mut().expect("class pushed above");
                assign_class(c, line, key, value)?;
            }
            Section::Field => {
                let c = m.classes.last_mut().expect("class pushed above");
                let f = c.fields.last_mut().expect("field pushed above");
                assign_field(f, line, key, value)?;
            }
            Section::Method => {
                let c = m.classes.last_mut().expect("class pushed above");
                let me = c.methods.last_mut().expect("method pushed above");
                assign_method(me, line, key, value)?;
            }
            Section::EnumValue => {
                let c = m.classes.last_mut().expect("class pushed above");
                let v = c.values.last_mut().expect("value pushed above");
                match key {
                    "name" => v.name = as_str(line, key, &value)?,
                    "doc" => v.doc = as_str(line, key, &value)?,
                    other => return err(line, format!("`{other}` is not a [[class.value]] key")),
                }
            }
        }
    }

    if m.version == 0 {
        return err(0, "manifest declares no `version`");
    }
    Ok(m)
}

fn open_class<'a>(
    m: &'a mut Manifest,
    line: usize,
    what: &str,
) -> Result<&'a mut Class, ParseError> {
    match m.classes.last_mut() {
        Some(c) => Ok(c),
        None => err(line, format!("[[class.{what}]] before any [[class]]")),
    }
}

fn assign_class(c: &mut Class, line: usize, key: &str, v: Value) -> Result<(), ParseError> {
    match key {
        "path" => c.path = as_str(line, key, &v)?,
        "extends" => c.extends = Some(as_str(line, key, &v)?),
        "kind" => {
            let s = as_str(line, key, &v)?;
            c.kind = Some(Kind::parse(&s).ok_or_else(|| ParseError {
                line,
                message: format!("`{s}` is not object | struct | enum"),
            })?);
        }
        "sealed" => c.sealed = as_bool(line, key, &v)?,
        "abstract" => c.is_abstract = as_bool(line, key, &v)?,
        "status" => c.status = as_status(line, &v)?,
        "doc" => c.doc = as_str(line, key, &v)?,
        other => return err(line, format!("`{other}` is not a [[class]] key")),
    }
    Ok(())
}

fn assign_field(f: &mut Field, line: usize, key: &str, v: Value) -> Result<(), ParseError> {
    match key {
        "name" => f.name = as_str(line, key, &v)?,
        "type" => f.ty = as_str(line, key, &v)?,
        "api" => f.api = as_bool(line, key, &v)?,
        "final" => f.is_final = as_bool(line, key, &v)?,
        "exposed" => f.exposed = as_bool(line, key, &v)?,
        "mutable" => f.mutable = as_bool(line, key, &v)?,
        "status" => f.status = as_status(line, &v)?,
        "doc" => f.doc = as_str(line, key, &v)?,
        "default" => f.default = Some(as_str(line, key, &v)?),
        other => return err(line, format!("`{other}` is not a [[class.field]] key")),
    }
    Ok(())
}

fn assign_method(me: &mut Method, line: usize, key: &str, v: Value) -> Result<(), ParseError> {
    match key {
        "name" => me.name = as_str(line, key, &v)?,
        "returns" => me.returns = as_str(line, key, &v)?,
        "api" => me.api = as_bool(line, key, &v)?,
        "final" => me.is_final = as_bool(line, key, &v)?,
        "abstract" => me.is_abstract = as_bool(line, key, &v)?,
        "hook" => me.hook = as_bool(line, key, &v)?,
        "status" => me.status = as_status(line, &v)?,
        "doc" => me.doc = as_str(line, key, &v)?,
        "default" => me.default = Some(as_str(line, key, &v)?),
        "args" => match v {
            Value::Params(p) => me.params = p,
            _ => return err(line, "`args` must be an array of inline tables"),
        },
        other => return err(line, format!("`{other}` is not a [[class.method]] key")),
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// scalars
// ---------------------------------------------------------------------------------------------

fn parse_value(line: usize, s: &str) -> Result<Value, ParseError> {
    if s == "true" {
        return Ok(Value::Bool(true));
    }
    if s == "false" {
        return Ok(Value::Bool(false));
    }
    if s.starts_with('"') {
        return Ok(Value::Str(parse_string(line, s)?));
    }
    if s.starts_with('[') {
        return Ok(Value::Params(parse_params(line, s)?));
    }
    match s.parse::<i64>() {
        Ok(n) => Ok(Value::Int(n)),
        Err(_) => err(
            line,
            format!("`{s}` is not a string, bool, integer, or arg list — floats and dates are deliberately not in the schema"),
        ),
    }
}

/// A double-quoted string with no escapes. The schema has no need for them, and refusing them keeps
/// the writer's job (M01) trivially reversible.
fn parse_string(line: usize, s: &str) -> Result<String, ParseError> {
    let body = match s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(b) => b,
        None => return err(line, "unterminated string"),
    };
    if body.contains('"') {
        return err(line, "escapes and inner quotes are not part of the schema");
    }
    Ok(body.to_string())
}

/// `[ { name = "ctx", type = "Ref<Context>" }, … ]` — the only array form the schema uses.
fn parse_params(line: usize, s: &str) -> Result<Vec<Param>, ParseError> {
    let inner = match s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        Some(b) => b.trim(),
        None => return err(line, "unterminated array"),
    };
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in inner.split('}') {
        let c = chunk.trim().trim_start_matches(',').trim();
        if c.is_empty() {
            continue;
        }
        let body = match c.strip_prefix('{') {
            Some(b) => b.trim(),
            None => return err(line, "expected `{ name = …, type = … }` in an arg list"),
        };
        let mut name = None;
        let mut ty = None;
        for pair in body.split(',') {
            let p = pair.trim();
            if p.is_empty() {
                continue;
            }
            let (k, val) = match p.split_once('=') {
                Some(x) => (x.0.trim(), x.1.trim()),
                None => return err(line, "expected `key = value` inside an arg"),
            };
            let v = parse_string(line, val)?;
            match k {
                "name" => name = Some(v),
                "type" => ty = Some(v),
                other => return err(line, format!("`{other}` is not an arg key")),
            }
        }
        match (name, ty) {
            (Some(name), Some(ty)) => out.push(Param { name, ty }),
            _ => return err(line, "an arg needs both `name` and `type`"),
        }
    }
    Ok(out)
}

fn as_str(line: usize, key: &str, v: &Value) -> Result<String, ParseError> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        _ => err(line, format!("`{key}` must be a string")),
    }
}

fn as_bool(line: usize, key: &str, v: &Value) -> Result<bool, ParseError> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => err(line, format!("`{key}` must be true or false")),
    }
}

fn as_int(line: usize, key: &str, v: &Value) -> Result<i64, ParseError> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => err(line, format!("`{key}` must be an integer")),
    }
}

fn as_status(line: usize, v: &Value) -> Result<Status, ParseError> {
    let s = as_str(line, "status", v)?;
    Status::parse(&s).ok_or_else(|| ParseError {
        line,
        message: format!("`{s}` is not proposed | stable | deprecated"),
    })
}
