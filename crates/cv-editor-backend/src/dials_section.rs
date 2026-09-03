//! **The `DIALS` section** — where a dial is *created*, and the only place.
//!
//! ⚠ **Setup and usage are separate surfaces.** A dial is created and configured on the thing that owns
//! it — a Schematic, or a Spine slot. The standalone Dials view **turns** knobs and creates nothing.
//! Conflating them would make *"where does this dial live"* unanswerable from the place you were looking
//! at it.
//!
//! # One row shape, six bodies
//!
//! ⚠ **`[ + ]` opens one row in place, and the `kind` dropdown swaps the rest of it.** Numeric fields
//! become an enum picker become an asset picker plus a row selector — so a developer learns *one* row
//! and gets six kinds, rather than learning six dialogs.
//!
//! ⚠ **Identity is the owner plus the name**, spelled `<ClassName>.<DialName>`. There is no separate id
//! field, and `Id=` on the block stays the ordinary block id — so renaming a dial is renaming a dial,
//! not a migration.

use cv_bindings::DialKind;
use std::fmt;

/// A dial being authored, before it is written to the document.
#[derive(Clone, Debug, PartialEq)]
pub struct DialDraft {
    /// The dial's name on its owner.
    pub name: String,
    /// Which of the six kinds.
    pub kind: DialKind,
    /// The developer's words.
    pub doc: String,
    /// The body, whose shape the kind decides.
    pub body: DialBody,
}

/// The part of the row the `kind` dropdown swaps.
#[derive(Clone, Debug, PartialEq)]
pub enum DialBody {
    /// `Type=` `Default=` `Min=` `Max=`.
    Number {
        ty: String,
        default: f64,
        min: f64,
        max: f64,
    },
    /// A hard `lo..hi`.
    Range { lo: f64, hi: f64 },
    /// ⚠ A **soft** pair. Spelled `SoftMin=`/`HardMax=` rather than `Min=`/`Max=` precisely so a reader
    /// cannot mistake it for a [`Range`](DialBody::Range).
    Adaptive { soft_min: f64, hard_max: f64 },
    /// `Enum=Enum'…'` `Default=`.
    Enum { path: String, default: String },
    /// One row of a curve table: `Asset=` `Row=`.
    Curve { asset: String, row: String },
    /// A whole table, evaluated at an axis: `Asset=` `Eval=`.
    Table { asset: String, eval: String },
}

impl DialBody {
    /// Which kind this body is.
    pub fn kind(&self) -> DialKind {
        match self {
            DialBody::Number { .. } => DialKind::Number,
            DialBody::Range { .. } => DialKind::Range,
            DialBody::Adaptive { .. } => DialKind::Adaptive,
            DialBody::Enum { .. } => DialKind::Enum,
            DialBody::Curve { .. } => DialKind::Curve,
            DialBody::Table { .. } => DialKind::Table,
        }
    }

    /// The body a freshly-switched `kind` dropdown starts with.
    ///
    /// ⚠ **Switching kind replaces the body rather than trying to carry values across.** A `Default=30`
    /// means nothing as a curve row, and a picker pre-filled with a number a developer never chose is
    /// worse than an empty one they have to fill.
    pub fn blank(kind: DialKind) -> DialBody {
        match kind {
            DialKind::Number => DialBody::Number {
                ty: "float".into(),
                default: 0.0,
                min: 0.0,
                max: 1.0,
            },
            DialKind::Range => DialBody::Range { lo: 0.0, hi: 1.0 },
            DialKind::Adaptive => DialBody::Adaptive {
                soft_min: 0.0,
                hard_max: 1.0,
            },
            DialKind::Enum => DialBody::Enum {
                path: String::new(),
                default: String::new(),
            },
            DialKind::Curve => DialBody::Curve {
                asset: String::new(),
                row: String::new(),
            },
            DialKind::Table => DialBody::Table {
                asset: String::new(),
                eval: String::new(),
            },
        }
    }
}

/// Why a draft could not be committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialDraftError {
    /// No name.
    Unnamed,
    /// A name that is not `snake_case`.
    ///
    /// ⚠ **A refusal here and a lint elsewhere**, and the asymmetry is deliberate: the naming lint is
    /// dismissible for content that already exists, but a dial being created *now* has no cost to
    /// naming correctly, and the id it produces is what host code will type forever.
    BadName { written: String },
    /// Two dials on one owner sharing a name.
    ///
    /// ⚠ **Identity is owner plus name**, so a duplicate is an ambiguous id rather than a cosmetic
    /// clash.
    Duplicate { name: String },
    /// A body a picker has not been filled in.
    Incomplete { field: &'static str },
    /// A `min` above a `max`.
    Inverted,
}

impl fmt::Display for DialDraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialDraftError::Unnamed => write!(f, "a dial needs a name"),
            DialDraftError::BadName { written } => write!(
                f,
                "{written} is not snake_case — the id this makes is what host code types forever"
            ),
            DialDraftError::Duplicate { name } => write!(
                f,
                "this owner already has a dial called {name}, and identity is owner plus name"
            ),
            DialDraftError::Incomplete { field } => write!(f, "{field} has not been chosen"),
            DialDraftError::Inverted => write!(f, "the low end is above the high end"),
        }
    }
}

impl std::error::Error for DialDraftError {}

impl DialDraft {
    /// A draft the `[ + ]` button opens.
    pub fn new(kind: DialKind) -> Self {
        DialDraft {
            name: String::new(),
            kind,
            doc: String::new(),
            body: DialBody::blank(kind),
        }
    }

    /// Name it.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Document it.
    pub fn documented(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// Give it a body.
    pub fn with(mut self, body: DialBody) -> Self {
        self.kind = body.kind();
        self.body = body;
        self
    }

    /// Switch the kind, which swaps the body.
    pub fn switch(mut self, kind: DialKind) -> Self {
        self.kind = kind;
        self.body = DialBody::blank(kind);
        self
    }

    /// Check the draft against the dials this owner already has.
    pub fn validate(&self, existing: &[&str]) -> Result<(), DialDraftError> {
        if self.name.is_empty() {
            return Err(DialDraftError::Unnamed);
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(DialDraftError::BadName {
                written: self.name.clone(),
            });
        }
        if existing.contains(&self.name.as_str()) {
            return Err(DialDraftError::Duplicate {
                name: self.name.clone(),
            });
        }
        match &self.body {
            DialBody::Number { min, max, .. } if min > max => Err(DialDraftError::Inverted),
            DialBody::Range { lo, hi } if lo > hi => Err(DialDraftError::Inverted),
            DialBody::Adaptive { soft_min, hard_max } if soft_min > hard_max => {
                Err(DialDraftError::Inverted)
            }
            DialBody::Enum { path, .. } if path.is_empty() => {
                Err(DialDraftError::Incomplete { field: "Enum" })
            }
            DialBody::Curve { asset, row } if asset.is_empty() || row.is_empty() => {
                Err(DialDraftError::Incomplete { field: "Asset/Row" })
            }
            DialBody::Table { asset, eval } if asset.is_empty() || eval.is_empty() => {
                Err(DialDraftError::Incomplete {
                    field: "Asset/Eval",
                })
            }
            _ => Ok(()),
        }
    }

    /// The `Begin Dial` block this draft serialises to.
    ///
    /// ⚠ **The same block a `.cvspine` writes**, because a dial is the same concept with a different
    /// owner. If these two ever diverged, *"one block both vocabularies share"* would be a claim rather
    /// than a fact — and the round-trip test in `cv-cvb` is what keeps it one.
    pub fn to_block(&self, id: &str) -> String {
        let mut out = format!(
            "Begin Dial Name=\"{}\" Kind={} Id={id}\n",
            self.name,
            kind_word(self.kind)
        );
        let mut line = |s: String| out.push_str(&format!("   {s}\n"));
        match &self.body {
            DialBody::Number {
                ty,
                default,
                min,
                max,
            } => {
                line(format!("Type={ty}"));
                line(format!("Default={}", number(*default)));
                line(format!("Min={}", number(*min)));
                line(format!("Max={}", number(*max)));
            }
            DialBody::Range { lo, hi } => {
                line(format!("Lo={}", number(*lo)));
                line(format!("Hi={}", number(*hi)));
            }
            DialBody::Adaptive { soft_min, hard_max } => {
                line(format!("SoftMin={}", number(*soft_min)));
                line(format!("HardMax={}", number(*hard_max)));
            }
            DialBody::Enum { path, default } => {
                line(format!("Enum=Enum'{path}'"));
                line(format!("Default={default}"));
            }
            DialBody::Curve { asset, row } => {
                line(format!("Asset=Asset'{asset}'"));
                line(format!("Row=\"{row}\""));
            }
            DialBody::Table { asset, eval } => {
                line(format!("Asset=Asset'{asset}'"));
                line(format!("Eval=\"{eval}\""));
            }
        }
        if !self.doc.is_empty() {
            line(format!("Doc=\"{}\"", self.doc));
        }
        out.push_str("End Dial\n");
        out
    }

    /// The id host code will type: `<ClassName>.<DialName>`.
    pub fn qualified_id(&self, owner: &str) -> String {
        let short = owner.rsplit('/').next().unwrap_or(owner);
        format!("{short}.{}", self.name)
    }
}

fn kind_word(k: DialKind) -> &'static str {
    match k {
        DialKind::Number => "Number",
        DialKind::Range => "Range",
        DialKind::Adaptive => "Adaptive",
        DialKind::Enum => "Enum",
        DialKind::Curve => "Curve",
        DialKind::Table => "Table",
    }
}

/// A number as the canonical writer would spell it.
fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_cvb::parse::parse;

    #[test]
    fn the_plus_button_opens_one_row_and_the_dropdown_swaps_its_body() {
        // ⚠ A developer learns one row and gets six kinds, rather than six dialogs.
        let draft = DialDraft::new(DialKind::Number);
        assert!(matches!(draft.body, DialBody::Number { .. }));

        let switched = draft.switch(DialKind::Curve);
        assert_eq!(switched.kind, DialKind::Curve);
        assert!(matches!(switched.body, DialBody::Curve { .. }));
    }

    #[test]
    fn switching_kind_replaces_the_body_rather_than_carrying_values_across() {
        // ⚠ A `Default=30` means nothing as a curve row, and a picker pre-filled with a number the
        // developer never chose is worse than an empty one.
        let numeric = DialDraft::new(DialKind::Number).with(DialBody::Number {
            ty: "float".into(),
            default: 30.0,
            min: 8.0,
            max: 200.0,
        });
        let curve = numeric.switch(DialKind::Curve);
        assert_eq!(
            curve.body,
            DialBody::Curve {
                asset: String::new(),
                row: String::new()
            }
        );
    }

    #[test]
    fn every_kind_has_a_blank_body_of_the_matching_shape() {
        for kind in DialKind::ALL {
            assert_eq!(DialBody::blank(kind).kind(), kind, "{kind}");
        }
        assert_eq!(DialKind::ALL.len(), 6);
    }

    #[test]
    fn a_number_dial_serialises_to_the_block_the_format_document_shows() {
        let draft = DialDraft::new(DialKind::Number)
            .named("length")
            .documented("how far the rope reaches")
            .with(DialBody::Number {
                ty: "float".into(),
                default: 30.0,
                min: 8.0,
                max: 200.0,
            });
        let block = draft.to_block("dial_04");
        assert!(block.starts_with("Begin Dial Name=\"length\" Kind=Number Id=dial_04\n"));
        assert!(block.contains("   Type=float\n"));
        assert!(block.contains("   Default=30.0\n"));
        assert!(block.contains("   Min=8.0\n"));
        assert!(block.contains("   Max=200.0\n"));
        assert!(block.contains("   Doc=\"how far the rope reaches\"\n"));
        assert!(block.ends_with("End Dial\n"));
    }

    #[test]
    fn every_kind_serialises_to_something_the_parser_reads_back() {
        // ⚠ The block a schematic writes is the block `cv-cvb` parses; a section that emitted anything
        // else would make the shared-block claim false the first time anyone opened the file.
        let cases = [
            DialBody::Number {
                ty: "float".into(),
                default: 1.0,
                min: 0.0,
                max: 2.0,
            },
            DialBody::Range { lo: 0.0, hi: 1.0 },
            DialBody::Adaptive {
                soft_min: 3.0,
                hard_max: 5.0,
            },
            DialBody::Enum {
                path: "/Core/ItemClass".into(),
                default: "PROGRESSION".into(),
            },
            DialBody::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            },
            DialBody::Table {
                asset: "/Content/Curves/progression.cvcurve".into(),
                eval: "depth".into(),
            },
        ];
        for body in cases {
            let kind = body.kind();
            let draft = DialDraft::new(kind).named("d").with(body);
            let doc = parse(&draft.to_block("dial_01")).expect("the block parses");
            assert_eq!(doc.kind, "Dial");
            assert_eq!(
                doc.header_get("Kind").map(ToString::to_string),
                Some(kind_word(kind).to_string()),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_curve_dial_writes_row_and_a_table_dial_writes_eval() {
        // ⚠ One row of a table and a whole table read at an axis are different things, and the two
        // spellings are how a reader tells them apart.
        let curve = DialDraft::new(DialKind::Curve)
            .named("wear")
            .with(DialBody::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            });
        assert!(curve.to_block("d").contains("Row=\"rate\""));

        let table = DialDraft::new(DialKind::Table)
            .named("pacing")
            .with(DialBody::Table {
                asset: "/Content/Curves/progression.cvcurve".into(),
                eval: "depth".into(),
            });
        let block = table.to_block("d");
        assert!(block.contains("Eval=\"depth\""));
        assert!(!block.contains("Row="));
    }

    #[test]
    fn an_adaptive_dial_is_spelled_so_it_cannot_be_read_as_a_range() {
        // ⚠ SoftMin/HardMax rather than Min/Max: one end is a preference and the other a ceiling.
        let block = DialDraft::new(DialKind::Adaptive)
            .named("room_count")
            .with(DialBody::Adaptive {
                soft_min: 3.0,
                hard_max: 5.0,
            })
            .to_block("d");
        assert!(block.contains("SoftMin=3.0"));
        assert!(block.contains("HardMax=5.0"));
        assert!(!block.contains("\n   Min="));
        assert!(!block.contains("\n   Max="));
    }

    #[test]
    fn the_id_is_the_owner_plus_the_name() {
        // ⚠ No separate id field, so renaming a dial is renaming a dial rather than a migration.
        let draft = DialDraft::new(DialKind::Number).named("length");
        assert_eq!(
            draft.qualified_id("/Content/Items/Hookshot"),
            "Hookshot.length"
        );
    }

    #[test]
    fn an_unnamed_or_badly_named_dial_is_refused_at_creation() {
        // ⚠ A lint elsewhere is dismissible; here the id is what host code types forever.
        assert_eq!(
            DialDraft::new(DialKind::Number).validate(&[]),
            Err(DialDraftError::Unnamed)
        );
        let err = DialDraft::new(DialKind::Number)
            .named("MaxLength")
            .validate(&[])
            .unwrap_err();
        assert!(matches!(err, DialDraftError::BadName { .. }));
        assert!(err.to_string().contains("host code types forever"));
    }

    #[test]
    fn two_dials_on_one_owner_may_not_share_a_name() {
        // ⚠ Identity is owner plus name, so a duplicate is an ambiguous id.
        let draft = DialDraft::new(DialKind::Number)
            .named("length")
            .with(DialBody::Number {
                ty: "float".into(),
                default: 1.0,
                min: 0.0,
                max: 2.0,
            });
        assert_eq!(draft.validate(&[]), Ok(()));
        assert_eq!(
            draft.validate(&["length"]),
            Err(DialDraftError::Duplicate {
                name: "length".into()
            })
        );
        assert_eq!(draft.validate(&["grade"]), Ok(()));
    }

    #[test]
    fn a_picker_that_was_never_filled_in_is_refused() {
        for (kind, field) in [
            (DialKind::Enum, "Enum"),
            (DialKind::Curve, "Asset/Row"),
            (DialKind::Table, "Asset/Eval"),
        ] {
            assert_eq!(
                DialDraft::new(kind).named("d").validate(&[]),
                Err(DialDraftError::Incomplete { field })
            );
        }
    }

    #[test]
    fn an_inverted_range_is_refused_in_every_shape_that_has_one() {
        for body in [
            DialBody::Number {
                ty: "float".into(),
                default: 0.0,
                min: 10.0,
                max: 1.0,
            },
            DialBody::Range { lo: 10.0, hi: 1.0 },
            DialBody::Adaptive {
                soft_min: 10.0,
                hard_max: 1.0,
            },
        ] {
            assert_eq!(
                DialDraft::new(body.kind())
                    .named("d")
                    .with(body)
                    .validate(&[]),
                Err(DialDraftError::Inverted)
            );
        }
    }
}
