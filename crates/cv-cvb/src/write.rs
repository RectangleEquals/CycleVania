//! **The canonical writer** — the same logical document serialises to identical bytes on any machine.
//!
//! ⚠ **That is a stronger claim than *"it round-trips"***, and it is the one that makes diffs and merges
//! usable: two authors making the same edit produce the same file, so a diff shows the edit rather than
//! the machine it was made on.
//!
//! | Rule | |
//! |---|---|
//! | **key order in a header** | fixed by the schema, not by authoring order |
//! | **`Pos=` last on a node header** | so a move touches one line at a predictable place |
//! | **positions snap to an 8-unit grid** | so nudges do not register in a diff at all |
//! | **floats** | fixed decimals, never scientific notation |
//! | **line endings** | `\n`, always |
//! | **indent** | three spaces per level |
//!
//! ⚠ **`Pos=` last is a layout rule with a logic consequence.** Position lives *in* the payload because
//! the clipboard is the format — a paste has to preserve arrangement — so the diff noise it would
//! otherwise cause is paid for here rather than by splitting the file in two.

use crate::parse::{Block, Line, Pin};
use crate::value::Value;
use std::fmt::Write as _;

/// Three spaces per level.
pub const CANONICAL_INDENT: &str = "   ";

/// Positions snap to this grid on write.
pub const POSITION_GRID: f64 = 8.0;

/// The fixed header key order.
///
/// ⚠ **A list rather than alphabetical**, because the meaningful order is the one a reader scans:
/// what the block *is*, then what it extends, then its identity. Alphabetical would put `Extends`
/// before `Path` and `Id` before both, which reads like noise.
const HEADER_ORDER: &[&str] = &[
    "Version",
    "Format",
    "Source",
    "Graph",
    "Path",
    "Name",
    "Variable",
    "From",
    "To",
    "Predecessor",
    "Successor",
    "Role",
    "Kind",
    "Scope",
    "AppliesTo",
    "Type",
    "Extends",
    "Op",
    "Id",
    "Pos",
];

/// The fixed pin key order.
///
/// ⚠ **Declaration order from the palette, not alphabetical.** A pin list a developer reads should
/// match the node they are looking at; sorting it would scramble every node in the project to satisfy a
/// rule nobody benefits from.
const PIN_ORDER: &[&str] = &["Name", "Dir", "Type", "Value", "To"];

fn rank(order: &[&str], key: &str) -> usize {
    order.iter().position(|k| *k == key).unwrap_or(order.len())
}

/// Write a block as canonical CVB.
pub fn write(block: &Block) -> String {
    let mut out = String::new();
    write_block(block, 0, &mut out);
    out
}

fn write_block(b: &Block, depth: usize, out: &mut String) {
    let pad = CANONICAL_INDENT.repeat(depth);
    let _ = write!(out, "{pad}Begin {}", b.kind);

    let mut header: Vec<&(Option<String>, Value)> = b.header.iter().collect();
    header.sort_by_key(|(k, _)| rank(HEADER_ORDER, k.as_deref().unwrap_or("")));
    for (k, v) in header {
        match k {
            Some(k) => {
                let _ = write!(out, " {k}={}", canonical_value(k, v));
            }
            None => {
                let _ = write!(out, " {v}");
            }
        }
    }
    out.push('\n');

    // Pairs, then pins, then nested blocks — each group in a stable order.
    //
    // ⚠ **Grouping rather than preserving authored order.** Two files that differ only in whether a
    // dial was declared above or below a component are the same document, and a canonical form that
    // kept the difference would put it in every diff.
    let mut pairs: Vec<(&String, &Value)> = Vec::new();
    let mut pins: Vec<&Pin> = Vec::new();
    let mut blocks: Vec<&Block> = Vec::new();
    for line in &b.lines {
        match line {
            Line::Pair { key, value } => pairs.push((key, value)),
            Line::Pin(p) => pins.push(p),
            Line::Block(nested) => blocks.push(nested),
        }
    }
    pairs.sort_by(|a, c| a.0.cmp(c.0));
    blocks.sort_by_key(|nested| block_sort_key(nested));

    let inner = CANONICAL_INDENT.repeat(depth + 1);
    for (k, v) in pairs {
        let _ = writeln!(out, "{inner}{k}={}", canonical_value(k, v));
    }
    for p in pins {
        let mut entries: Vec<&(String, Value)> = p.entries.iter().collect();
        entries.sort_by_key(|(k, _)| rank(PIN_ORDER, k));
        let body: Vec<String> = entries
            .iter()
            .map(|(k, v)| format!("{k}={}", canonical_value(k, v)))
            .collect();
        let _ = writeln!(out, "{inner}Pin ({})", body.join(", "));
    }
    for nested in blocks {
        write_block(nested, depth + 1, out);
    }

    let _ = writeln!(out, "{pad}End {}", b.kind);
}

/// Blocks sort by kind, then by id — so the order is a property of the document, not of the session.
fn block_sort_key(b: &Block) -> (String, String) {
    let id = b
        .header_get("Id")
        .map(|v| v.to_string())
        .unwrap_or_default();
    (b.kind.clone(), id)
}

/// Snap and format a value, applying the position rule where the key says to.
fn canonical_value(key: &str, v: &Value) -> String {
    if key == "Pos" || key == "Size" {
        return snap(v).to_string();
    }
    v.to_string()
}

/// Snap a position tuple to the grid.
///
/// ⚠ **Applied on write, never on read.** Snapping at parse time would make a file that came from
/// another tool change meaning by being opened, and the grid is a diff-noise measure rather than a
/// semantic one.
fn snap(v: &Value) -> Value {
    match v {
        Value::Number { value, fractional } => Value::Number {
            value: (value / POSITION_GRID).round() * POSITION_GRID,
            fractional: *fractional,
        },
        Value::Tuple(entries) => {
            Value::Tuple(entries.iter().map(|(k, e)| (k.clone(), snap(e))).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn round(src: &str) -> String {
        write(&parse(src).unwrap())
    }

    #[test]
    fn writing_is_idempotent_so_the_second_pass_changes_nothing() {
        let src = "Begin X Id=x Version=1\n   B=2\n   A=1\nEnd X\n";
        let once = round(src);
        assert_eq!(once, round(&once), "canonical form is a fixed point");
    }

    #[test]
    fn header_keys_are_ordered_by_the_schema_and_not_by_authoring() {
        let a = round("Begin Node Id=n Op=core.branch Pos=(0,0)\nEnd Node\n");
        let b = round("Begin Node Pos=(0,0) Op=core.branch Id=n\nEnd Node\n");
        assert_eq!(a, b, "authoring order must not survive into the bytes");
        assert!(a.starts_with("Begin Node Op=core.branch Id=n Pos=(0,0)"));
    }

    #[test]
    fn pos_is_last_on_the_node_header() {
        // ⚠ So a move touches one line at a predictable place.
        let out = round("Begin Node Pos=(16,24) Id=n Op=core.branch\nEnd Node\n");
        let head = out.lines().next().unwrap();
        assert!(head.trim_end().ends_with("Pos=(16,24)"), "{head}");
    }

    #[test]
    fn positions_snap_to_the_grid_so_a_nudge_does_not_register() {
        let out = round("Begin Node Id=n Op=x Pos=(-317,3)\nEnd Node\n");
        assert!(out.contains("Pos=(-320,0)"), "{out}");
        let nudged = round("Begin Node Id=n Op=x Pos=(-321,2)\nEnd Node\n");
        assert_eq!(out, nudged, "two nudges inside one grid cell are one file");
    }

    #[test]
    fn snapping_happens_on_write_and_never_on_read() {
        // ⚠ Otherwise opening a file from another tool would change its meaning.
        let b = parse("Begin Node Id=n Op=x Pos=(-317,3)\nEnd Node\n").unwrap();
        assert_eq!(
            b.header_get("Pos").unwrap().to_string(),
            "(-317,3)",
            "the parsed document keeps what was written"
        );
    }

    #[test]
    fn pin_keys_follow_the_palette_order_rather_than_the_alphabet() {
        let out =
            round("Begin Node Id=n\n   Pin (To=(n2.a), Type=bool, Name=out, Dir=Out)\nEnd Node\n");
        let pin = out.lines().find(|l| l.contains("Pin (")).unwrap();
        let name = pin.find("Name=").unwrap();
        let dir = pin.find("Dir=").unwrap();
        let ty = pin.find("Type=").unwrap();
        let to = pin.find("To=").unwrap();
        assert!(name < dir && dir < ty && ty < to, "{pin}");
    }

    #[test]
    fn nested_blocks_sort_by_kind_then_id() {
        let out = round(
            "Begin S\n   Begin Node Id=n_02\n   End Node\n   Begin Node Id=n_01\n   End Node\n\
             \n   Begin Component Id=c_01\n   End Component\nEnd S\n",
        );
        let order: Vec<&str> = out
            .lines()
            .filter(|l| l.trim_start().starts_with("Begin ") && !l.starts_with("Begin S"))
            .collect();
        assert!(order[0].contains("Component"));
        assert!(order[1].contains("n_01"));
        assert!(order[2].contains("n_02"));
    }

    #[test]
    fn the_same_document_written_two_ways_produces_identical_bytes() {
        // ⚠ The claim the whole module exists for.
        let a = "Begin Slot Name=\"x\" Id=s_01\n   Shape (MaxDegree=4, MinDegree=2)\nEnd Slot\n";
        let b = "Begin Slot Id=s_01 Name=\"x\"\n   Shape (MaxDegree=4, MinDegree=2)\nEnd Slot\n";
        assert_eq!(round(a), round(b));
    }

    #[test]
    fn floats_keep_their_fixed_decimal_form_through_a_round_trip() {
        let out = round("Begin X\n   A=0.5\n   B=30.0\n   C=1\nEnd X\n");
        assert!(out.contains("A=0.5"));
        assert!(out.contains("B=30.0"));
        assert!(out.contains("C=1"), "an integer stays an integer: {out}");
        // Scan the values, not the whole document - "Begin" contains an `e`.
        for line in out.lines().filter(|l| l.contains('=')) {
            let value = line.split_once('=').unwrap().1;
            assert!(
                !value.contains('e') && !value.contains('E'),
                "never scientific notation: {line}"
            );
        }
    }

    #[test]
    fn indentation_is_three_spaces_per_level() {
        let out = round("Begin A\n   Begin B\n      X=1\n   End B\nEnd A\n");
        assert!(out.contains("\n   Begin B\n"));
        assert!(out.contains("\n      X=1\n"));
    }

    #[test]
    fn every_line_ends_with_a_single_newline_and_never_a_carriage_return() {
        let out = round("Begin A\n   X=1\nEnd A\n");
        assert!(!out.contains('\r'));
        assert!(out.ends_with("End A\n"));
    }
}
