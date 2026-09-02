//! **The three formats** — and the clipboard rule that keeps them apart.
//!
//! ⚠ **Three formats, three `Format=` values, three validators, three clipboards, one parser.** The
//! parser is shared because the *notation* is what is being reused; everything above it is separate
//! because the *languages* are different.
//!
//! # `Format=` is not decoration
//!
//! ⚠ **Without it, CVB's self-describing property produces a payload that parses cleanly and means
//! nothing.** A Schematic fragment pastes only into a schematic graph; a Spine fragment only into a
//! spine. Pasting across is **rejected with the reason**, never silently coerced, because the two
//! vocabularies differ even where the spelling matches.
//!
//! # The op namespace *is* the vocabulary boundary, and it is checked asymmetrically
//!
//! `core.instances_of` inside a `Begin Fill` and `fill.scatter` inside a `Begin Graph` both **parse
//! perfectly**, which is exactly why the check has to exist. A block that parses and means nothing is
//! the failure mode a shared notation invites.
//!
//! ⚠ **Only the fill palette can be checked positively.** A schematic'''s op set is *"the whole generated
//! palette"* — the format document'''s own example puts `array.is_empty` beside `core.branch` — so
//! requiring one prefix there would reject the specification. The fill palette is the small, closed,
//! deliberately-gateless one, so a spine is checked against what it **must** be and a schematic against
//! what it certainly **must not**.
//!
//! # A pasted dial node resolves by qualified id or not at all
//!
//! ⚠ **Never silently rebound to a same-named dial on the destination.** `Hookshot.length#dial` pasted
//! into a schematic that does not declare that dial lands **unresolved and flagged**, exactly as a paste
//! referencing a missing class does. Rebinding would make a fragment mean something different where it
//! lands — and *"a fragment means the same thing wherever it goes"* is the property the whole
//! travel-through-prose idea rests on.

use crate::parse::Block;
use crate::value::Value;
use std::fmt;

/// One of the three formats written in CVB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    /// `.cvs` — an authored class, graphs with configuration around them.
    Schematic,
    /// `.cvspine` — a spine template, configuration with graphs nested inside.
    Spine,
    /// `.cvstate` — a bounded state machine.
    StateGraph,
}

impl Format {
    /// All three.
    pub const ALL: [Format; 3] = [Format::Schematic, Format::Spine, Format::StateGraph];

    /// The `Format=` spelling, which is also the root block's type token.
    ///
    /// ⚠ **One name for both**, so a fragment's `Format=` and its block names agree instead of having
    /// to be cross-checked.
    pub fn name(self) -> &'static str {
        match self {
            Format::Schematic => "Schematic",
            Format::Spine => "Spine",
            Format::StateGraph => "StateGraph",
        }
    }

    /// Parse a `Format=` value.
    pub fn from_name(name: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.name() == name)
    }

    /// The file extension.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Schematic => ".cvs",
            Format::Spine => ".cvspine",
            Format::StateGraph => ".cvstate",
        }
    }

    /// The prefix this format's ops **must** carry, when its palette is a closed set.
    ///
    /// ⚠ **Only the fill palette is closed, and the asymmetry is the design's rather than an
    /// oversight.** A schematic's op set is *"the whole generated palette"* — `core.branch`,
    /// `array.is_empty`, and every namespace the manifest generates — so requiring one prefix there
    /// would reject the format document's own worked example. The fill palette is the small, closed,
    /// deliberately-gateless one, so it is the half that can be checked positively.
    ///
    /// A state graph returns `None` because its nodes are **states**, not operations. There is no op
    /// set to namespace, and inventing an empty one would suggest a palette that does not exist.
    pub fn required_op_prefix(self) -> Option<&'static str> {
        match self {
            Format::Spine => Some("fill."),
            Format::Schematic | Format::StateGraph => None,
        }
    }

    /// Prefixes that belong to another format and must not appear here.
    ///
    /// ⚠ **The open palette is guarded negatively.** A schematic cannot be checked against a list of
    /// what it may contain, but it can be checked against what it certainly may not: a `fill.` op in a
    /// schematic graph parses perfectly and means nothing, which is the failure a shared notation
    /// invites.
    pub fn foreign_op_prefixes(self) -> &'static [&'static str] {
        match self {
            Format::Schematic => &["fill."],
            Format::Spine => &[],
            Format::StateGraph => &["fill.", "core."],
        }
    }

    /// The block that holds nodes in this format.
    pub fn node_block(self) -> Option<&'static str> {
        match self {
            Format::Schematic => Some("Graph"),
            Format::Spine => Some("Fill"),
            Format::StateGraph => None,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a document or a paste was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// The root block is not one of the three.
    UnknownRoot { kind: String },
    /// A fragment carries no `Format=`.
    ///
    /// ⚠ **Refused rather than inferred from the contents.** Guessing is what produces a payload that
    /// parses cleanly and means nothing.
    FragmentWithoutFormat,
    /// A fragment's `Format=` names nothing.
    UnknownFormat { written: String },
    /// A fragment from one format pasted into another.
    CrossFormatPaste { from: Format, into: Format },
    /// An op whose namespace belongs to another format.
    WrongOpNamespace {
        op: String,
        expected: &'static str,
        format: Format,
    },
    /// A node block this format does not have.
    WrongNodeBlock { block: String, format: Format },
    /// A dial node whose dial the destination does not declare.
    UnresolvedDial { op: String },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::UnknownRoot { kind } => write!(
                f,
                "Begin {kind} is not a CVB document root — the three are Schematic, Spine and StateGraph"
            ),
            FormatError::FragmentWithoutFormat => write!(
                f,
                "a fragment must declare Format=, or it parses cleanly and means nothing"
            ),
            FormatError::UnknownFormat { written } => {
                write!(f, "Format={written} names no format")
            }
            FormatError::CrossFormatPaste { from, into } => write!(
                f,
                "a {from} fragment does not paste into a {into}: near-identical syntax, different \
                 meaning — the vocabularies differ even where the spelling matches"
            ),
            FormatError::WrongOpNamespace {
                op,
                expected,
                format,
            } if *expected == "no" => write!(
                f,
                "{op} is not a {format} op — a state graph's nodes are states, not operations, and \
                 it has no palette at all"
            ),
            FormatError::WrongOpNamespace {
                op,
                expected,
                format,
            } => write!(
                f,
                "{op} does not belong in a {format} — {expected}* is another format's palette, and an \
                 op from it parses perfectly while meaning nothing here"
            ),
            FormatError::WrongNodeBlock { block, format } => {
                write!(f, "a {format} has no Begin {block}")
            }
            FormatError::UnresolvedDial { op } => write!(
                f,
                "{op} refers to a dial this document does not declare — left unresolved and flagged, \
                 never rebound to a same-named dial, which would make the fragment mean something \
                 different where it landed"
            ),
        }
    }
}

impl std::error::Error for FormatError {}

/// Which format a parsed document is.
pub fn format_of(root: &Block) -> Result<Format, FormatError> {
    Format::from_name(&root.kind).ok_or(FormatError::UnknownRoot {
        kind: root.kind.clone(),
    })
}

/// The format a fragment came from.
pub fn fragment_format(fragment: &Block) -> Result<Format, FormatError> {
    let Some(v) = fragment.header_get("Format") else {
        return Err(FormatError::FragmentWithoutFormat);
    };
    let written = match v {
        Value::Ident(s) => s.clone(),
        Value::Quoted(s) => s.clone(),
        other => other.to_string(),
    };
    Format::from_name(&written).ok_or(FormatError::UnknownFormat { written })
}

/// May this fragment paste into a document of that format?
pub fn may_paste(fragment: &Block, into: Format) -> Result<(), FormatError> {
    let from = fragment_format(fragment)?;
    if from != into {
        return Err(FormatError::CrossFormatPaste { from, into });
    }
    Ok(())
}

/// Every op in a document belongs to that format's palette.
///
/// ⚠ **Checked because both spellings parse.** A `core.` op inside a `Begin Fill` is a perfectly valid
/// CVB document that means nothing, and a shared notation makes that mistake easy rather than rare.
pub fn check_ops(root: &Block, format: Format) -> Result<(), FormatError> {
    check_ops_in(root, format)
}

fn check_ops_in(b: &Block, format: Format) -> Result<(), FormatError> {
    if let Some(Value::Ident(op)) = b.header_get("Op") {
        // ⚠ A dial get node is qualified by its owner and ends in `#dial`, so it is namespaced by
        // *ownership* rather than by palette. Checking it against an op prefix would reject every
        // legitimate dial read.
        if !op.ends_with("#dial") {
            if let Some(required) = format.required_op_prefix() {
                if !op.starts_with(required) {
                    return Err(FormatError::WrongOpNamespace {
                        op: op.clone(),
                        expected: required,
                        format,
                    });
                }
            }
            if format.required_op_prefix().is_none() && format == Format::StateGraph {
                return Err(FormatError::WrongOpNamespace {
                    op: op.clone(),
                    expected: "no",
                    format,
                });
            }
            for foreign in format.foreign_op_prefixes() {
                if op.starts_with(foreign) {
                    return Err(FormatError::WrongOpNamespace {
                        op: op.clone(),
                        expected: foreign,
                        format,
                    });
                }
            }
        }
    }
    for child in b.children() {
        check_ops_in(child, format)?;
    }
    Ok(())
}

/// Every node block in a document is one this format has.
pub fn check_node_blocks(root: &Block, format: Format) -> Result<(), FormatError> {
    let foreign: Vec<&'static str> = Format::ALL
        .into_iter()
        .filter(|f| *f != format)
        .filter_map(Format::node_block)
        .filter(|b| Some(*b) != format.node_block())
        .collect();
    check_blocks_in(root, &foreign, format)
}

fn check_blocks_in(b: &Block, foreign: &[&'static str], format: Format) -> Result<(), FormatError> {
    for child in b.children() {
        if foreign.contains(&child.kind.as_str()) {
            return Err(FormatError::WrongNodeBlock {
                block: child.kind.clone(),
                format,
            });
        }
        check_blocks_in(child, foreign, format)?;
    }
    Ok(())
}

/// The qualified dial a get node reads, if it is one.
///
/// The op is `<owner>.<dial>#dial`, so identity is the **owner plus the name** — there is no separate
/// id field, and `Id=` stays the ordinary block id.
pub fn dial_read(op: &str) -> Option<(&str, &str)> {
    let base = op.strip_suffix("#dial")?;
    let (owner, name) = base.rsplit_once('.')?;
    Some((owner, name))
}

/// Does the destination declare every dial the pasted nodes read?
///
/// ⚠ **Unresolved and flagged, never rebound.** A same-named dial on the destination is a *different*
/// dial, and binding to it would make the fragment mean something different where it landed.
pub fn check_dials(fragment: &Block, declared: &[&str]) -> Result<(), FormatError> {
    let mut ops = Vec::new();
    collect_ops(fragment, &mut ops);
    for op in ops {
        if let Some((owner, name)) = dial_read(&op) {
            let qualified = format!("{owner}.{name}");
            if !declared.contains(&qualified.as_str()) {
                return Err(FormatError::UnresolvedDial { op });
            }
        }
    }
    Ok(())
}

fn collect_ops(b: &Block, out: &mut Vec<String>) {
    if let Some(Value::Ident(op)) = b.header_get("Op") {
        out.push(op.clone());
    }
    for child in b.children() {
        collect_ops(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn frag(format: &str, op: &str) -> Block {
        parse(&format!(
            "Begin Fragment Version=1 Format={format} Source=/Content/x\n   \
             Begin Node Id=n_0001 Op={op} Pos=(0,0)\n   End Node\nEnd Fragment\n"
        ))
        .unwrap()
    }

    #[test]
    fn each_format_names_itself_the_same_way_its_root_block_does() {
        for f in Format::ALL {
            assert_eq!(Format::from_name(f.name()), Some(f));
            assert!(f.extension().starts_with(".cv"));
        }
        assert_eq!(Format::ALL.len(), 3);
    }

    #[test]
    fn a_fragment_pastes_into_its_own_format_and_is_refused_by_the_others() {
        let schematic = frag("Schematic", "core.branch");
        assert_eq!(may_paste(&schematic, Format::Schematic), Ok(()));
        assert_eq!(
            may_paste(&schematic, Format::Spine),
            Err(FormatError::CrossFormatPaste {
                from: Format::Schematic,
                into: Format::Spine
            })
        );
        assert!(may_paste(&schematic, Format::StateGraph).is_err());
    }

    #[test]
    fn the_refusal_says_why_rather_than_coercing() {
        let err = may_paste(&frag("Spine", "fill.scatter"), Format::Schematic).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("does not paste into"));
        assert!(
            text.contains("vocabularies differ"),
            "the reason must be in the message: {text}"
        );
    }

    #[test]
    fn a_fragment_without_a_format_is_refused_rather_than_guessed_at() {
        // ⚠ Guessing produces a payload that parses cleanly and means nothing.
        let b = parse("Begin Fragment Version=1 Source=/Content/x\nEnd Fragment\n").unwrap();
        assert_eq!(fragment_format(&b), Err(FormatError::FragmentWithoutFormat));
    }

    #[test]
    fn an_unknown_format_name_is_refused() {
        let b = parse("Begin Fragment Version=1 Format=Blueprint\nEnd Fragment\n").unwrap();
        assert_eq!(
            fragment_format(&b),
            Err(FormatError::UnknownFormat {
                written: "Blueprint".into()
            })
        );
    }

    #[test]
    fn an_op_from_another_palette_parses_and_is_still_refused() {
        // ⚠ Both spellings parse — that is exactly why the check exists.
        let spine = parse(
            "Begin Spine Version=1 Path=/Content/Spines/A Id=s\n   \
             Begin Segment From=\"a\" To=\"b\" Id=seg\n      \
             Begin Fill Name=\"f\" Id=fil\n         \
             Begin Node Id=n Op=core.instances_of Pos=(0,0)\n         End Node\n      \
             End Fill\n   End Segment\nEnd Spine\n",
        )
        .unwrap();
        let err = check_ops(&spine, Format::Spine).unwrap_err();
        assert_eq!(
            err,
            FormatError::WrongOpNamespace {
                op: "core.instances_of".into(),
                expected: "fill.",
                format: Format::Spine
            }
        );
        assert!(err.to_string().contains("meaning nothing here"));
        assert!(err.to_string().contains("fill.*"));
    }

    #[test]
    fn a_schematic_accepts_the_whole_generated_palette_and_not_just_one_prefix() {
        // ⚠ The format document's own example uses `array.is_empty` beside `core.branch`. A schematic's
        // op set is *the whole generated palette*, so a single required prefix would reject the
        // specification.
        assert_eq!(Format::Schematic.required_op_prefix(), None);
        let sch = parse(
            "Begin Schematic Version=1 Id=s\n   Begin Graph Name=\"g\" Id=grf\n      \
             Begin Node Id=n1 Op=core.branch Pos=(0,0)\n      End Node\n      \
             Begin Node Id=n2 Op=array.is_empty Pos=(80,0)\n      End Node\n   \
             End Graph\nEnd Schematic\n",
        )
        .unwrap();
        assert_eq!(check_ops(&sch, Format::Schematic), Ok(()));
    }

    #[test]
    fn a_fill_op_in_a_schematic_is_refused_the_same_way() {
        let sch = parse(
            "Begin Schematic Version=1 Path=/Content/x Id=s\n   \
             Begin Graph Name=\"g\" Id=grf\n      \
             Begin Node Id=n Op=fill.scatter Pos=(0,0)\n      End Node\n   End Graph\nEnd Schematic\n",
        )
        .unwrap();
        assert!(matches!(
            check_ops(&sch, Format::Schematic),
            Err(FormatError::WrongOpNamespace { .. })
        ));
    }

    #[test]
    fn a_state_graph_has_no_ops_at_all_and_any_op_is_foreign() {
        // ⚠ Its nodes are states, not operations. An empty palette is not the same as no palette.
        assert_eq!(Format::StateGraph.required_op_prefix(), None);
        assert_eq!(Format::StateGraph.node_block(), None);
        let clean = parse(
            "Begin StateGraph Version=1 Path=/Content/States/W Id=stg\n   \
             Begin State Name=\"low\" Id=s1\n      Initial=true\n   End State\nEnd StateGraph\n",
        )
        .unwrap();
        assert_eq!(check_ops(&clean, Format::StateGraph), Ok(()));

        let dirty = parse(
            "Begin StateGraph Version=1 Id=stg\n   \
             Begin Node Id=n Op=core.branch\n   End Node\nEnd StateGraph\n",
        )
        .unwrap();
        assert!(check_ops(&dirty, Format::StateGraph).is_err());
    }

    #[test]
    fn a_graph_block_in_a_spine_is_refused_and_a_fill_block_in_a_schematic_too() {
        let spine = parse(
            "Begin Spine Version=1 Id=s\n   Begin Graph Name=\"g\" Id=grf\n   End Graph\nEnd Spine\n",
        )
        .unwrap();
        assert_eq!(
            check_node_blocks(&spine, Format::Spine),
            Err(FormatError::WrongNodeBlock {
                block: "Graph".into(),
                format: Format::Spine
            })
        );

        let sch = parse(
            "Begin Schematic Version=1 Id=s\n   Begin Fill Name=\"f\" Id=fil\n   End Fill\nEnd Schematic\n",
        )
        .unwrap();
        assert!(check_node_blocks(&sch, Format::Schematic).is_err());
    }

    #[test]
    fn a_dial_read_is_qualified_by_its_owner() {
        assert_eq!(
            dial_read("/Content/Items/Hookshot.length#dial"),
            Some(("/Content/Items/Hookshot", "length"))
        );
        assert_eq!(
            dial_read("core.branch"),
            None,
            "an ordinary op is not a dial"
        );
        assert_eq!(
            dial_read("length"),
            None,
            "an unqualified name is not one either"
        );
    }

    #[test]
    fn a_pasted_dial_node_resolves_by_qualified_id_or_not_at_all() {
        // ⚠ Never rebound to a same-named dial: that would make the fragment mean something different
        // where it landed.
        let f = frag("Schematic", "/Content/Items/Hookshot.length#dial");
        assert_eq!(
            check_dials(&f, &["/Content/Items/Hookshot.length"]),
            Ok(()),
            "the destination declares it"
        );
        let err = check_dials(&f, &["/Content/Items/Grapple.length"]).unwrap_err();
        assert!(matches!(err, FormatError::UnresolvedDial { .. }));
        assert!(
            err.to_string().contains("never rebound"),
            "the message must rule out the tempting fix: {err}"
        );
        assert!(check_dials(&f, &[]).is_err());
    }

    #[test]
    fn a_dial_read_is_not_checked_against_the_op_namespace() {
        // ⚠ It is namespaced by ownership, not by palette — checking it against `core.` would reject
        // every legitimate dial read.
        let sch = parse(
            "Begin Schematic Version=1 Id=s\n   Begin Graph Name=\"g\" Id=grf\n      \
             Begin Node Id=n Op=/Content/Items/Hookshot.length#dial Pos=(0,0)\n      End Node\n   \
             End Graph\nEnd Schematic\n",
        )
        .unwrap();
        assert_eq!(check_ops(&sch, Format::Schematic), Ok(()));
    }

    #[test]
    fn an_unknown_root_block_is_not_a_cvb_document() {
        let b = parse("Begin Blueprint Version=1\nEnd Blueprint\n").unwrap();
        assert_eq!(
            format_of(&b),
            Err(FormatError::UnknownRoot {
                kind: "Blueprint".into()
            })
        );
        for f in Format::ALL {
            let doc = parse(&format!("Begin {f} Version=1\nEnd {f}\n")).unwrap();
            assert_eq!(format_of(&doc), Ok(f));
        }
    }
}
