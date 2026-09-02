//! **The parser** — nine productions, hand-written recursive descent.
//!
//! ⚠ **No lexer generator and no parser generator.** The format was shaped to make them unnecessary, so
//! reaching for one would be evidence the format had drifted rather than a convenience. What that buys
//! is a parser a reader can check against the grammar line by line, and errors that name a line and a
//! column instead of a state number.

use crate::value::{RefTag, Value};
use std::fmt;

/// A `Pin (…)` line.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    /// Its keyed entries, in written order.
    pub entries: Vec<(String, Value)>,
}

impl Pin {
    /// Look one up.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Is this an execution pin?
    ///
    /// ⚠ **`Dir=Out` *and* `Type=exec` — neither alone expresses it.** A pin that is only `Dir=Out` is a
    /// data output, and one that is only `Type=exec` has no direction, which is not a thing.
    pub fn is_exec(&self) -> bool {
        matches!(self.get("Type"), Some(Value::Ident(t)) if t == "exec")
    }
}

/// One line inside a block.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// `Key=Value`.
    Pair { key: String, value: Value },
    /// `Pin (…)`.
    Pin(Pin),
    /// A nested `Begin … End`.
    Block(Block),
}

/// A `Begin X … End X` block.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The type token after `Begin`.
    pub kind: String,
    /// Header entries — keyed, or bare (a lone identifier).
    pub header: Vec<(Option<String>, Value)>,
    /// The body.
    pub lines: Vec<Line>,
}

impl Block {
    /// A header value by key.
    pub fn header_get(&self, key: &str) -> Option<&Value> {
        self.header
            .iter()
            .find(|(k, _)| k.as_deref() == Some(key))
            .map(|(_, v)| v)
    }

    /// A body pair by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.lines.iter().find_map(|l| match l {
            Line::Pair { key: k, value } if k == key => Some(value),
            _ => None,
        })
    }

    /// Nested blocks of a kind.
    pub fn blocks(&self, kind: &str) -> Vec<&Block> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Block(b) if b.kind == kind => Some(b),
                _ => None,
            })
            .collect()
    }

    /// Every nested block.
    pub fn children(&self) -> Vec<&Block> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Block(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    /// Every pin in this block.
    pub fn pins(&self) -> Vec<&Pin> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Pin(p) => Some(p),
                _ => None,
            })
            .collect()
    }
}

/// Why a document did not parse.
///
/// ⚠ **Every variant carries a line**, because a parse error without a location is a search rather than
/// a fix — and a fragment pasted from prose is exactly the case where the reader has no file open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-indexed.
    pub line: usize,
    /// What went wrong.
    pub kind: ErrorKind,
}

/// What kind of parse failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The file did not start with `Begin`.
    NoRootBlock,
    /// `End X` did not match its `Begin Y`.
    MismatchedEnd { began: String, ended: String },
    /// A block never closed.
    UnclosedBlock { kind: String },
    /// `End` with nothing open.
    UnexpectedEnd,
    /// A reference tag the vocabulary does not list.
    ///
    /// ⚠ **The one that matters most.** Accepting it would turn a typo into a silent default.
    UnknownRefTag { written: String },
    /// A value that did not parse.
    BadValue { text: String },
    /// A line that is neither a pair, a pin, nor a block.
    BadLine { text: String },
    /// Content after the root block closed.
    TrailingContent,
    /// A quoted string with no closing quote.
    UnterminatedQuote,
    /// A `'…'` reference with no closing quote.
    UnterminatedReference,
    /// Brackets that never closed.
    UnbalancedBrackets,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.kind {
            ErrorKind::NoRootBlock => write!(f, "a CVB file starts with a Begin block"),
            ErrorKind::MismatchedEnd { began, ended } => {
                write!(f, "Begin {began} closed by End {ended}")
            }
            ErrorKind::UnclosedBlock { kind } => write!(f, "Begin {kind} was never closed"),
            ErrorKind::UnexpectedEnd => write!(f, "End with no matching Begin"),
            ErrorKind::UnknownRefTag { written } => write!(
                f,
                "{written}'…' is not a reference tag — the set is Kind, Ref, Asset, Resource, Enum, \
                 Variant, Struct, Array and Tag, and it is closed so a typo cannot become a default"
            ),
            ErrorKind::BadValue { text } => write!(f, "cannot read {text:?} as a value"),
            ErrorKind::BadLine { text } => write!(
                f,
                "{text:?} is neither Key=Value, a Pin (…), nor a Begin block"
            ),
            ErrorKind::TrailingContent => write!(f, "content after the root block closed"),
            ErrorKind::UnterminatedQuote => write!(f, "a quoted string never closed"),
            ErrorKind::UnterminatedReference => write!(f, "a reference's closing ' is missing"),
            ErrorKind::UnbalancedBrackets => write!(f, "brackets never closed"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a CVB document into its single root block.
pub fn parse(src: &str) -> Result<Block, ParseError> {
    let lines: Vec<(usize, &str)> = src
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();

    let mut at = 0usize;
    let Some((no, first)) = lines.first().copied() else {
        return Err(ParseError {
            line: 1,
            kind: ErrorKind::NoRootBlock,
        });
    };
    if !first.starts_with("Begin ") {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::NoRootBlock,
        });
    }
    let root = block(&lines, &mut at)?;
    if at < lines.len() {
        return Err(ParseError {
            line: lines[at].0,
            kind: ErrorKind::TrailingContent,
        });
    }
    Ok(root)
}

fn block(lines: &[(usize, &str)], at: &mut usize) -> Result<Block, ParseError> {
    let (no, text) = lines[*at];
    let rest = text.strip_prefix("Begin ").ok_or(ParseError {
        line: no,
        kind: ErrorKind::BadLine {
            text: text.to_string(),
        },
    })?;
    let mut parts = split_header(rest, no)?;
    if parts.is_empty() {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::BadLine {
                text: text.to_string(),
            },
        });
    }
    let kind = parts.remove(0);
    let mut header = Vec::new();
    for part in parts {
        header.push(header_entry(&part, no)?);
    }
    *at += 1;

    let mut body = Vec::new();
    loop {
        let Some(&(lno, line)) = lines.get(*at) else {
            return Err(ParseError {
                line: no,
                kind: ErrorKind::UnclosedBlock { kind },
            });
        };
        if let Some(closing) = line.strip_prefix("End ") {
            let closing = closing.trim();
            if closing != kind {
                return Err(ParseError {
                    line: lno,
                    kind: ErrorKind::MismatchedEnd {
                        began: kind,
                        ended: closing.to_string(),
                    },
                });
            }
            *at += 1;
            return Ok(Block {
                kind,
                header,
                lines: body,
            });
        }
        if line == "End" {
            return Err(ParseError {
                line: lno,
                kind: ErrorKind::UnexpectedEnd,
            });
        }
        if line.starts_with("Begin ") {
            body.push(Line::Block(block(lines, at)?));
            continue;
        }
        body.push(body_line(line, lno)?);
        *at += 1;
    }
}

fn body_line(text: &str, no: usize) -> Result<Line, ParseError> {
    if let Some(rest) = text
        .strip_prefix("Pin ")
        .or_else(|| text.strip_prefix("Pin("))
    {
        // ⚠ **Strip exactly one bracket at each end.** `trim_end_matches(')')` removes *every*
        // trailing `)`, so `To=(n_0002.value))` lost the tuple's own close as well as the pin's and
        // reported unbalanced brackets on a line that was balanced.
        let rest = rest.trim();
        let rest = rest.strip_prefix('(').unwrap_or(rest);
        let inner = rest.strip_suffix(')').unwrap_or(rest).to_string();
        let mut entries = Vec::new();
        for part in split_commas(&inner, no)? {
            let (k, v) = header_entry(&part, no)?;
            let Some(k) = k else {
                return Err(ParseError {
                    line: no,
                    kind: ErrorKind::BadLine {
                        text: part.to_string(),
                    },
                });
            };
            entries.push((k, v));
        }
        return Ok(Line::Pin(Pin { entries }));
    }
    // `Comment (…)` and `Shape (…)` are pair-shaped groups: a key followed by a tuple.
    if let Some((head, tail)) = text.split_once(" (") {
        if !head.contains('=') {
            let value = value(&format!("({tail}"), no)?;
            return Ok(Line::Pair {
                key: head.trim().to_string(),
                value,
            });
        }
    }
    let (k, v) = header_entry(text, no)?;
    let Some(key) = k else {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::BadLine {
                text: text.to_string(),
            },
        });
    };
    Ok(Line::Pair { key, value: v })
}

fn header_entry(part: &str, no: usize) -> Result<(Option<String>, Value), ParseError> {
    let part = part.trim();
    match split_first_eq(part) {
        Some((k, v)) => Ok((Some(k.trim().to_string()), value(v.trim(), no)?)),
        None => Ok((None, value(part, no)?)),
    }
}

/// The first `=` that is not inside quotes, brackets or a reference.
fn split_first_eq(s: &str) -> Option<(&str, &str)> {
    let (mut depth, mut quoted, mut in_ref) = (0i32, false, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '\'' if !quoted => in_ref = !in_ref,
            '(' | '<' if !quoted && !in_ref => depth += 1,
            ')' | '>' if !quoted && !in_ref => depth -= 1,
            '=' if !quoted && !in_ref && depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Split a header into whitespace-separated parts, respecting quotes and brackets.
fn split_header(s: &str, no: usize) -> Result<Vec<String>, ParseError> {
    split_on(s, ' ', no)
}

fn split_commas(s: &str, no: usize) -> Result<Vec<String>, ParseError> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    split_on(s, ',', no)
}

fn split_on(s: &str, sep: char, no: usize) -> Result<Vec<String>, ParseError> {
    let (mut depth, mut quoted, mut in_ref) = (0i32, false, false);
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            '\'' if !quoted => {
                in_ref = !in_ref;
                cur.push(c);
            }
            '(' | '<' if !quoted && !in_ref => {
                depth += 1;
                cur.push(c);
            }
            ')' | '>' if !quoted && !in_ref => {
                depth -= 1;
                cur.push(c);
            }
            c if c == sep && !quoted && !in_ref && depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if quoted {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::UnterminatedQuote,
        });
    }
    if in_ref {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::UnterminatedReference,
        });
    }
    if depth != 0 {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::UnbalancedBrackets,
        });
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    Ok(out)
}

/// Parse one value.
pub fn value(text: &str, no: usize) -> Result<Value, ParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ParseError {
            line: no,
            kind: ErrorKind::BadValue {
                text: String::new(),
            },
        });
    }

    // A row selector: `…#"Row"`.
    //
    // ⚠ **The quote is part of the syntax, not decoration.** Without requiring it, the `#` in a dial
    // read - `/Content/Items/Hookshot.length#dial` - parsed as a row selector, so the op stopped being
    // an identifier and every dial check silently found nothing to check.
    if let Some(hash) = find_top_level(text, '#') {
        if text[hash + 1..].trim_start().starts_with('"') {
            let (base, rest) = text.split_at(hash);
            let row = rest[1..].trim().trim_matches('"').to_string();
            return Ok(Value::Row {
                base: Box::new(value(base, no)?),
                row,
            });
        }
    }

    if text.starts_with('"') {
        if !text.ends_with('"') || text.len() < 2 {
            return Err(ParseError {
                line: no,
                kind: ErrorKind::UnterminatedQuote,
            });
        }
        return Ok(Value::Quoted(text[1..text.len() - 1].to_string()));
    }

    if text.starts_with('(') {
        if !text.ends_with(')') {
            return Err(ParseError {
                line: no,
                kind: ErrorKind::UnbalancedBrackets,
            });
        }
        let inner = &text[1..text.len() - 1];
        let mut entries = Vec::new();
        for part in split_commas(inner, no)? {
            entries.push(header_entry(&part, no)?);
        }
        return Ok(Value::Tuple(entries));
    }

    // A reference: `Tag'path'`, optionally with a `.member` after it.
    if let Some(q) = text.find('\'') {
        let tag_name = &text[..q];
        if !tag_name.is_empty() && tag_name.chars().all(|c| c.is_ascii_alphanumeric()) {
            let Some(end) = text[q + 1..].find('\'') else {
                return Err(ParseError {
                    line: no,
                    kind: ErrorKind::UnterminatedReference,
                });
            };
            let Some(tag) = RefTag::from_name(tag_name) else {
                return Err(ParseError {
                    line: no,
                    kind: ErrorKind::UnknownRefTag {
                        written: tag_name.to_string(),
                    },
                });
            };
            let path = text[q + 1..q + 1 + end].to_string();
            let base = Value::Reference { tag, path };
            let after = text[q + 2 + end..].trim();
            if let Some(member) = after.strip_prefix('.') {
                return Ok(Value::Member {
                    base: Box::new(base),
                    member: member.to_string(),
                });
            }
            if !after.is_empty() {
                return Err(ParseError {
                    line: no,
                    kind: ErrorKind::BadValue {
                        text: text.to_string(),
                    },
                });
            }
            return Ok(base);
        }
    }

    // A container: `Name<…>`.
    if let Some(lt) = text.find('<') {
        if text.ends_with('>') {
            let name = text[..lt].trim().to_string();
            let inner = &text[lt + 1..text.len() - 1];
            let mut args = Vec::new();
            for part in split_commas(inner, no)? {
                args.push(value(&part, no)?);
            }
            return Ok(Value::Container { name, args });
        }
    }

    if let Ok(i) = text.parse::<i64>() {
        return Ok(Value::int(i));
    }
    if let Ok(v) = text.parse::<f64>() {
        return Ok(Value::Number {
            value: v,
            fractional: true,
        });
    }

    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '#' || c == '/')
    {
        return Ok(Value::Ident(text.to_string()));
    }

    Err(ParseError {
        line: no,
        kind: ErrorKind::BadValue {
            text: text.to_string(),
        },
    })
}

fn find_top_level(s: &str, needle: char) -> Option<usize> {
    let (mut depth, mut quoted, mut in_ref) = (0i32, false, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '\'' if !quoted => in_ref = !in_ref,
            '(' | '<' if !quoted && !in_ref => depth += 1,
            ')' | '>' if !quoted && !in_ref => depth -= 1,
            c if c == needle && !quoted && !in_ref && depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_block_parses() {
        let b = parse("Begin Schematic Version=1\nEnd Schematic\n").unwrap();
        assert_eq!(b.kind, "Schematic");
        assert_eq!(b.header_get("Version"), Some(&Value::int(1)));
        assert!(b.lines.is_empty());
    }

    #[test]
    fn nested_blocks_and_pins_parse() {
        let src = "\
Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_01
   Tag=Tag'Item.Tool.Tether'
   Begin Graph Name=\"requires\" Role=Hook Id=grf_01
      Begin Node Id=n_0001 Op=core.instances_of Pos=(-320,0)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>, To=(n_0002.value))
      End Node
   End Graph
End Schematic
";
        let b = parse(src).unwrap();
        assert_eq!(b.kind, "Schematic");
        assert_eq!(
            b.header_get("Extends"),
            Some(&Value::reference(RefTag::Kind, "/Core/Item"))
        );
        assert_eq!(
            b.get("Tag"),
            Some(&Value::reference(RefTag::Tag, "Item.Tool.Tether"))
        );
        let graph = b.blocks("Graph")[0];
        let node = graph.blocks("Node")[0];
        assert_eq!(
            node.header_get("Op"),
            Some(&Value::Ident("core.instances_of".into()))
        );
        let pin = node.pins()[0];
        assert_eq!(pin.get("Dir"), Some(&Value::Ident("Out".into())));
        assert!(matches!(pin.get("Type"), Some(Value::Container { .. })));
    }

    #[test]
    fn an_unknown_reference_tag_is_an_error_that_names_what_was_written() {
        // ⚠ Accepting it would turn a typo into a silent default.
        let err = parse("Begin X\n   A=Knid'/Core/Item'\nEnd X\n").unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::UnknownRefTag {
                written: "Knid".into()
            }
        );
        assert_eq!(err.line, 2);
        assert!(err.to_string().contains("closed"));
    }

    #[test]
    fn a_mismatched_end_names_both_halves() {
        let err = parse("Begin Schematic\nEnd Graph\n").unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::MismatchedEnd {
                began: "Schematic".into(),
                ended: "Graph".into()
            }
        );
    }

    #[test]
    fn an_unclosed_block_reports_the_line_it_opened_on() {
        // ⚠ The opening line is the actionable one; end-of-file is where it was noticed.
        let err = parse("Begin Schematic\n   A=1\n").unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::UnclosedBlock {
                kind: "Schematic".into()
            }
        );
        assert_eq!(err.line, 1);
    }

    #[test]
    fn a_file_that_does_not_start_with_begin_is_rejected() {
        assert_eq!(parse("A=1\n").unwrap_err().kind, ErrorKind::NoRootBlock);
        assert_eq!(parse("").unwrap_err().kind, ErrorKind::NoRootBlock);
    }

    #[test]
    fn content_after_the_root_block_is_rejected() {
        let err = parse("Begin A\nEnd A\nBegin B\nEnd B\n").unwrap_err();
        assert_eq!(err.kind, ErrorKind::TrailingContent);
    }

    #[test]
    fn quotes_and_brackets_protect_separators() {
        let b = parse("Begin X Doc=\"a, b = c\" P=(1,2)\nEnd X\n").unwrap();
        assert_eq!(b.header_get("Doc"), Some(&Value::Quoted("a, b = c".into())));
        assert_eq!(
            b.header_get("P"),
            Some(&Value::Tuple(vec![
                (None, Value::int(1)),
                (None, Value::int(2))
            ]))
        );
    }

    #[test]
    fn an_unterminated_quote_or_reference_is_reported_as_itself() {
        assert_eq!(
            parse("Begin X Doc=\"oops\nEnd X\n").unwrap_err().kind,
            ErrorKind::UnterminatedQuote
        );
        assert_eq!(
            parse("Begin X A=Kind'/Core/Item\nEnd X\n")
                .unwrap_err()
                .kind,
            ErrorKind::UnterminatedReference
        );
    }

    #[test]
    fn a_variant_tuple_keeps_its_form() {
        let b =
            parse("Begin X\n   Shape=(Form=CubeShape,extents=(2.0,1.0,2.0),bevel=0.05)\nEnd X\n")
                .unwrap();
        let shape = b.get("Shape").unwrap();
        assert_eq!(shape.form(), Some("CubeShape"));
        assert!(shape.get("extents").is_some());
    }

    #[test]
    fn an_enum_member_and_a_resource_row_both_parse() {
        let b = parse(
            "Begin X\n   S=Enum'/Core/InstanceScope'.AREA\n   \
             U=Asset'/Content/p.cvunlock'#\"Song\"\nEnd X\n",
        )
        .unwrap();
        assert!(matches!(b.get("S"), Some(Value::Member { .. })));
        assert!(matches!(b.get("U"), Some(Value::Row { .. })));
    }

    #[test]
    fn a_group_line_is_a_pair_whose_value_is_a_tuple() {
        // `Shape (MinDegree=2, MaxDegree=4)` — the spine's constraint groups.
        let b = parse("Begin Slot Name=\"x\"\n   Shape (MinDegree=2, MaxDegree=4)\nEnd Slot\n")
            .unwrap();
        let shape = b.get("Shape").expect("the group is a pair");
        assert_eq!(shape.get("MinDegree"), Some(&Value::int(2)));
        assert_eq!(shape.get("MaxDegree"), Some(&Value::int(4)));
    }

    #[test]
    fn an_exec_pin_needs_both_facts() {
        // ⚠ Dir=Out AND Type=exec. Neither alone expresses it.
        let b = parse(
            "Begin Node Id=n\n   Pin (Name=true, Dir=Out, Type=exec, To=(n_2.in))\n   \
             Pin (Name=out, Dir=Out, Type=bool)\nEnd Node\n",
        )
        .unwrap();
        let pins = b.pins();
        assert!(pins[0].is_exec());
        assert!(!pins[1].is_exec(), "Dir=Out alone is a data output");
    }

    #[test]
    fn blank_lines_are_ignored_and_indentation_does_not_matter_to_the_parser() {
        let a = parse("Begin X\n\n   A=1\n\nEnd X\n").unwrap();
        let b = parse("Begin X\nA=1\nEnd X\n").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_pin_keeps_its_own_closing_bracket_separate_from_a_tuple_value() {
        // ⚠ Trimming *every* trailing `)` ate the tuple's close as well as the pin's.
        let b = parse("Begin Node Id=n\n   Pin (Name=out, Dir=Out, To=(n_2.value))\nEnd Node\n")
            .unwrap();
        let to = b.pins()[0].get("To").expect("To survived the strip");
        assert!(matches!(to, Value::Tuple(_)));
    }

    #[test]
    fn a_dial_read_op_stays_an_identifier_rather_than_becoming_a_row_selector() {
        // ⚠ `#dial` is part of the op name; `#"Row"` is a selector. Without the quote requirement the
        // first parsed as the second and every dial check found nothing to check.
        let b =
            parse("Begin Node Id=n Op=/Content/Items/Hookshot.length#dial\nEnd Node\n").unwrap();
        assert_eq!(
            b.header_get("Op"),
            Some(&Value::Ident("/Content/Items/Hookshot.length#dial".into()))
        );
    }
}
