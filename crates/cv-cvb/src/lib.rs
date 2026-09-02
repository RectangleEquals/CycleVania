//! **CVB — the CycleVania Block notation.** One parser, one canonical writer, three formats.
//!
//! ```text
//! Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_8f3a
//!    Begin Node Id=n_0001 Op=core.instances_of Pos=(-320,0)
//!       Pin (Name=out, Dir=Out, Type=bool, To=(n_0002.cond))
//!    End Node
//! End Schematic
//! ```
//!
//! ⚠ **A notation, not a format.** `.cvs`, `.cvspine` and `.cvstate` are three *separate formats*
//! written in it, with separate vocabularies, separate validators and **separate clipboards**. The
//! parser is shared because the notation is what is being reused; everything above it is separate
//! because the languages are different.
//!
//! # Nine productions, and no generator
//!
//! ⚠ **Hand-written recursive descent, on purpose.** The format was shaped to make a lexer or parser
//! generator unnecessary, so reaching for one is a signal the format has drifted rather than a
//! convenience. The grammar is small enough to hold in your head:
//!
//! ```ebnf
//! file        = block ;
//! block       = "Begin" type header NEWLINE { line } "End" type NEWLINE ;
//! header      = { WS ( pair | bare ) } ;
//! line        = pair | pin | block ;
//! pair        = key "=" value ;
//! pin         = "Pin" WS "(" pair { "," pair } ")" ;
//! value       = number | quoted | ident | tuple | typedref | typeexpr ;
//! tuple       = "(" [ value { "," value } ] ")" ;
//! typedref    = tag "'" path "'" ;
//! ```
//!
//! # The type-token vocabulary is closed
//!
//! ⚠ **A parser that accepts a token the vocabulary does not list is how a typo becomes a silent
//! default.** `Kind'…'`, `Ref'…'`, `Asset'…'`, `Resource'…'`, `Enum'…'`, `Variant'…'`, `Struct'…'`,
//! `Array'…'` and `Tag'…'` are the reference tags; anything else with the same shape is an error that
//! names what was written and what was expected.
//!
//! # Round-tripping is a fixed point, not a similarity
//!
//! ⚠ **parse → write → parse must reach the same document**, and the *bytes* must match on any machine.
//! That is what content-derived ids, fixed key order, 8-unit position snapping and fixed-decimal floats
//! are all for: two authors making the same edit produce the same file, so a diff shows the edit rather
//! than the machine it was made on.

#![forbid(unsafe_code)]

pub mod format;
pub mod parse;
pub mod value;
pub mod write;

pub use format::{Format, FormatError};
pub use parse::{parse, Block, Line, ParseError, Pin};
pub use value::{RefTag, Value};
pub use write::{write, CANONICAL_INDENT, POSITION_GRID};

/// This crate's version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }
}
