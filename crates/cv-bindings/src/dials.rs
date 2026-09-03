//! **The dial interface** — `list` · `get` · `set` · `setSource`.
//!
//! ⚠ **This is the same interface the editor's Dials panel drives its UI from**, deliberately exposed to
//! host code **so a shipped game has the same fine-grained control the editor does**. The editor is not
//! allowed a private channel, and the way to keep that true is for there to be only one surface.
//!
//! # It works in a cooked build
//!
//! ⚠ **Dials are inputs, not content.** Nothing about cooking freezes them, so a host that needs tuning
//! to move after shipping uses this rather than a patchable asset — which is the same override channel a
//! curve table points at.
//!
//! # Setting a dial regenerates the world
//!
//! ⚠ **A changed dial is a different recipe**: the fingerprint moves and the next generate produces a
//! different world. There is no incremental path — partial re-application would leave decisions made
//! against the old value, which no seed would explain.
//!
//! # `set` and `setSource` are two calls, and the split is the reason the second exists
//!
//! ⚠ **Swapping a constant for a curve changes the dial's `kind`**, which `set(id, value)` has no way to
//! express: its signature takes a value, and a curve is not one.

use std::collections::BTreeMap;
use std::fmt;

/// What kind of thing a dial is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DialKind {
    /// A single number with bounds.
    Number,
    /// A hard `lo..hi` pair.
    Range,
    /// ⚠ A **soft** pair — `soft_min` is a preference and `hard_max` a ceiling.
    Adaptive,
    /// A choice from a declared enum.
    Enum,
    /// One row of a curve table.
    Curve,
    /// A whole table, evaluated at an axis.
    Table,
}

impl DialKind {
    /// All six.
    pub const ALL: [DialKind; 6] = [
        DialKind::Number,
        DialKind::Range,
        DialKind::Adaptive,
        DialKind::Enum,
        DialKind::Curve,
        DialKind::Table,
    ];

    /// The name the bindings use.
    pub fn name(self) -> &'static str {
        match self {
            DialKind::Number => "NUMBER",
            DialKind::Range => "RANGE",
            DialKind::Adaptive => "ADAPTIVE",
            DialKind::Enum => "ENUM",
            DialKind::Curve => "CURVE",
            DialKind::Table => "TABLE",
        }
    }
}

impl fmt::Display for DialKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A dial's value, in whichever of the six shapes it has.
#[derive(Clone, Debug, PartialEq)]
pub enum DialValue {
    Number(f64),
    /// A hard pair.
    Range {
        lo: f64,
        hi: f64,
    },
    /// ⚠ A soft pair. Named differently from [`DialValue::Range`] on purpose: a widget that drew them
    /// as one slider would lie about which end is a preference and which is a ceiling.
    Adaptive {
        soft_min: f64,
        hard_max: f64,
    },
    /// A value name from the declared enum.
    Enum(String),
    /// One row of a curve table.
    Curve {
        asset: String,
        row: String,
    },
    /// A whole table, read at an axis.
    Table {
        asset: String,
        axis: String,
    },
}

impl DialValue {
    /// Which kind this value is.
    pub fn kind(&self) -> DialKind {
        match self {
            DialValue::Number(_) => DialKind::Number,
            DialValue::Range { .. } => DialKind::Range,
            DialValue::Adaptive { .. } => DialKind::Adaptive,
            DialValue::Enum(_) => DialKind::Enum,
            DialValue::Curve { .. } => DialKind::Curve,
            DialValue::Table { .. } => DialKind::Table,
        }
    }

    /// Is this a source swap rather than a plain value?
    ///
    /// ⚠ **The two `set` calls differ by exactly this.** A curve or a table replaces where the number
    /// *comes from*; a number, range, adaptive pair or enum replaces the number itself.
    pub fn is_source(&self) -> bool {
        matches!(self, DialValue::Curve { .. } | DialValue::Table { .. })
    }
}

/// What a widget may offer.
///
/// ⚠ **What may be offered, not what must be satisfied.** A value outside its bounds is a **warning and
/// not a refusal**: content may have authored a default outside a range it later narrowed, and refusing
/// to report the dial would hide the mistake rather than surface it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DialBounds {
    /// The low end, for `NUMBER` and `RANGE`.
    pub min: Option<f64>,
    /// The high end.
    pub max: Option<f64>,
    /// The preference, for `ADAPTIVE`.
    pub soft_min: Option<f64>,
    /// The ceiling.
    pub hard_max: Option<f64>,
    /// The enum's path, for `ENUM`.
    pub enum_path: Option<String>,
    /// Its values.
    pub enum_values: Vec<String>,
}

impl DialBounds {
    /// Bounds for a number.
    pub fn number(min: f64, max: f64) -> Self {
        DialBounds {
            min: Some(min),
            max: Some(max),
            ..DialBounds::default()
        }
    }

    /// Bounds for an adaptive pair.
    pub fn adaptive(soft_min: f64, hard_max: f64) -> Self {
        DialBounds {
            soft_min: Some(soft_min),
            hard_max: Some(hard_max),
            ..DialBounds::default()
        }
    }

    /// Bounds for an enum.
    pub fn enumerated(path: impl Into<String>, values: impl IntoIterator<Item = String>) -> Self {
        DialBounds {
            enum_path: Some(path.into()),
            enum_values: values.into_iter().collect(),
            ..DialBounds::default()
        }
    }

    /// Does this value sit inside what a widget would offer?
    pub fn admits(&self, v: &DialValue) -> bool {
        match v {
            DialValue::Number(n) => {
                self.min.is_none_or(|m| *n >= m) && self.max.is_none_or(|m| *n <= m)
            }
            DialValue::Range { lo, hi } => {
                lo <= hi && self.min.is_none_or(|m| *lo >= m) && self.max.is_none_or(|m| *hi <= m)
            }
            DialValue::Adaptive { soft_min, hard_max } => soft_min <= hard_max,
            DialValue::Enum(name) => self.enum_values.is_empty() || self.enum_values.contains(name),
            DialValue::Curve { .. } | DialValue::Table { .. } => true,
        }
    }
}

/// Where a dial's effective value came from.
///
/// ⚠ **Exists so a developer can tell *why* a value is what it is.** An effective value that differs
/// from the default with nothing saying who changed it is the shape of an afternoon lost — and with a
/// scoped override in play, *"the number in the panel is not the number this room uses"* is otherwise
/// invisible.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialSource {
    /// What the content authored.
    #[default]
    Authored,
    /// A host set it, world-wide.
    Host,
    /// A host set it for one scope.
    Scoped,
}

impl DialSource {
    /// The name the bindings use.
    pub fn name(self) -> &'static str {
        match self {
            DialSource::Authored => "AUTHORED",
            DialSource::Host => "HOST",
            DialSource::Scoped => "SCOPED",
        }
    }
}

impl fmt::Display for DialSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Everything both the editor panel and a host need about one dial.
#[derive(Clone, Debug, PartialEq)]
pub struct DialMeta {
    /// `<ClassName>.<DialName>` — identity, and the only handle.
    pub id: String,
    /// The class path, so a panel can group without parsing the id.
    pub owner: String,
    /// The dial's own name.
    pub name: String,
    /// What kind it is.
    pub kind: DialKind,
    /// The developer's words.
    pub doc: String,
    /// What a widget may offer.
    pub bounds: DialBounds,
    /// ⚠ What the content authored. **Present alongside `effective` because neither is derivable from
    /// the other** — a panel needs this to render a *reset*.
    pub default: DialValue,
    /// What the next generate will actually use.
    pub effective: DialValue,
    /// Where `effective` came from.
    pub source: DialSource,
    /// The scope an override applies to, when `source` is `SCOPED`.
    pub scope: Option<String>,
}

impl DialMeta {
    /// A dial as content authored it.
    pub fn authored(
        owner: impl Into<String>,
        name: impl Into<String>,
        default: DialValue,
        bounds: DialBounds,
    ) -> Self {
        let (owner, name) = (owner.into(), name.into());
        let short = owner.rsplit('/').next().unwrap_or(&owner).to_string();
        DialMeta {
            id: format!("{short}.{name}"),
            owner,
            name,
            kind: default.kind(),
            doc: String::new(),
            bounds,
            effective: default.clone(),
            default,
            source: DialSource::Authored,
            scope: None,
        }
    }

    /// Give it the developer's words.
    pub fn documented(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// Has a host changed this from what content authored?
    pub fn is_overridden(&self) -> bool {
        self.source != DialSource::Authored
    }

    /// ⚠ **Is the effective value outside what a widget would offer?**
    ///
    /// Reported rather than refused — see [`DialBounds`].
    pub fn is_out_of_bounds(&self) -> bool {
        !self.bounds.admits(&self.effective)
    }
}

/// Why a dial call did not take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialError {
    /// No dial with that id.
    Unknown { id: String },
    /// A value of the wrong kind for that dial.
    ///
    /// ⚠ **A kind change is a `setSource`, never a `set`.** Silently accepting a curve through `set`
    /// would make the two calls indistinguishable and the split pointless.
    WrongKind {
        id: String,
        expected: DialKind,
        given: DialKind,
    },
    /// `set` was handed a source, or `set_source` a plain value.
    WrongCall {
        id: String,
        use_instead: &'static str,
    },
}

impl fmt::Display for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialError::Unknown { id } => write!(f, "no dial {id}"),
            DialError::WrongKind {
                id,
                expected,
                given,
            } => write!(f, "{id} is a {expected} dial; {given} was given"),
            DialError::WrongCall { id, use_instead } => write!(
                f,
                "{id}: use {use_instead} — swapping a constant for a curve changes the dial's kind, \
                 which set(id, value) has no way to express"
            ),
        }
    }
}

impl std::error::Error for DialError {}

/// The `project.dials` object.
#[derive(Clone, Debug, Default)]
pub struct Dials {
    entries: BTreeMap<String, DialMeta>,
    /// Bumped whenever anything changes, so a host can see that the recipe moved.
    revision: u64,
}

impl Dials {
    /// An empty set.
    pub fn new() -> Self {
        Dials::default()
    }

    /// Declare a dial, as loading content does.
    pub fn declare(&mut self, meta: DialMeta) {
        self.entries.insert(meta.id.clone(), meta);
        self.revision += 1;
    }

    /// Every dial, sorted by id.
    ///
    /// ⚠ **Sorted rather than in load order.** A panel's row order must not depend on which folder the
    /// project happened to scan first.
    pub fn list(&self) -> Vec<&DialMeta> {
        self.entries.values().collect()
    }

    /// Every dial belonging to one owner, which is how a panel groups.
    pub fn of_owner(&self, owner: &str) -> Vec<&DialMeta> {
        self.entries.values().filter(|d| d.owner == owner).collect()
    }

    /// One dial.
    pub fn get(&self, id: &str) -> Result<&DialMeta, DialError> {
        self.entries
            .get(id)
            .ok_or_else(|| DialError::Unknown { id: id.to_string() })
    }

    /// Set a dial's value, optionally for one scope.
    pub fn set(
        &mut self,
        id: &str,
        value: DialValue,
        scope: Option<&str>,
    ) -> Result<(), DialError> {
        if value.is_source() {
            return Err(DialError::WrongCall {
                id: id.to_string(),
                use_instead: "setSource",
            });
        }
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| DialError::Unknown { id: id.to_string() })?;
        if value.kind() != entry.kind {
            return Err(DialError::WrongKind {
                id: id.to_string(),
                expected: entry.kind,
                given: value.kind(),
            });
        }
        entry.effective = value;
        entry.source = match scope {
            Some(_) => DialSource::Scoped,
            None => DialSource::Host,
        };
        entry.scope = scope.map(str::to_string);
        self.revision += 1;
        Ok(())
    }

    /// Swap where a dial's value comes from.
    ///
    /// ⚠ **This is the call that changes a dial's `kind`.** It is what a curve table's override channel
    /// runs through, and it is why `set` cannot do the job.
    pub fn set_source(&mut self, id: &str, source: DialValue) -> Result<(), DialError> {
        if !source.is_source() {
            return Err(DialError::WrongCall {
                id: id.to_string(),
                use_instead: "set",
            });
        }
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| DialError::Unknown { id: id.to_string() })?;
        entry.kind = source.kind();
        entry.effective = source;
        entry.source = DialSource::Host;
        entry.scope = None;
        self.revision += 1;
        Ok(())
    }

    /// Put a dial back to what content authored.
    ///
    /// ⚠ **Possible only because `default` is carried.** A shape holding only the effective value could
    /// not offer this at all.
    pub fn reset(&mut self, id: &str) -> Result<(), DialError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| DialError::Unknown { id: id.to_string() })?;
        entry.effective = entry.default.clone();
        entry.kind = entry.default.kind();
        entry.source = DialSource::Authored;
        entry.scope = None;
        self.revision += 1;
        Ok(())
    }

    /// How many dials.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Nothing declared.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A counter that moves whenever anything here changes.
    ///
    /// ⚠ **What a host watches to know the recipe moved.** A changed dial is a different recipe, so the
    /// next `generate` produces a different world — and this is the cheapest honest way to say so
    /// without recomputing a fingerprint.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Every dial whose effective value is outside its bounds.
    pub fn out_of_bounds(&self) -> Vec<&DialMeta> {
        self.entries
            .values()
            .filter(|d| d.is_out_of_bounds())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dials() -> Dials {
        let mut d = Dials::new();
        d.declare(
            DialMeta::authored(
                "/Content/Items/Hookshot",
                "length",
                DialValue::Number(30.0),
                DialBounds::number(8.0, 200.0),
            )
            .documented("how far the rope reaches"),
        );
        d.declare(DialMeta::authored(
            "/Content/Items/Hookshot",
            "grade",
            DialValue::Enum("PROGRESSION".into()),
            DialBounds::enumerated(
                "/Core/ItemClass",
                ["PROGRESSION".to_string(), "USEFUL".to_string()],
            ),
        ));
        d.declare(DialMeta::authored(
            "/Content/Spines/Ascent",
            "room_count",
            DialValue::Adaptive {
                soft_min: 3.0,
                hard_max: 5.0,
            },
            DialBounds::adaptive(3.0, 5.0),
        ));
        d
    }

    #[test]
    fn the_id_is_class_name_dot_dial_name() {
        let d = dials();
        assert!(d.get("Hookshot.length").is_ok());
        assert_eq!(
            d.get("Hookshot.length").unwrap().owner,
            "/Content/Items/Hookshot"
        );
    }

    #[test]
    fn list_is_sorted_rather_than_in_load_order() {
        // ⚠ A panel's row order must not depend on which folder the project scanned first.
        let dials = dials();
        let ids: Vec<&str> = dials.list().iter().map(|d| d.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn a_panel_can_group_by_owner_without_parsing_the_id() {
        let d = dials();
        assert_eq!(d.of_owner("/Content/Items/Hookshot").len(), 2);
        assert_eq!(d.of_owner("/Content/Spines/Ascent").len(), 1);
    }

    #[test]
    fn default_and_effective_are_both_carried_so_reset_is_possible() {
        // ⚠ Neither is derivable from the other. Only-effective makes reset impossible.
        let mut d = dials();
        d.set("Hookshot.length", DialValue::Number(45.0), None)
            .unwrap();
        let meta = d.get("Hookshot.length").unwrap();
        assert_eq!(meta.effective, DialValue::Number(45.0));
        assert_eq!(meta.default, DialValue::Number(30.0));
        assert!(meta.is_overridden());

        d.reset("Hookshot.length").unwrap();
        let meta = d.get("Hookshot.length").unwrap();
        assert_eq!(meta.effective, DialValue::Number(30.0));
        assert!(!meta.is_overridden());
    }

    #[test]
    fn source_says_who_changed_it() {
        // ⚠ Without it, an effective value differing from the default has nothing saying why.
        let mut d = dials();
        assert_eq!(
            d.get("Hookshot.length").unwrap().source,
            DialSource::Authored
        );

        d.set("Hookshot.length", DialValue::Number(45.0), None)
            .unwrap();
        assert_eq!(d.get("Hookshot.length").unwrap().source, DialSource::Host);

        d.set("Hookshot.length", DialValue::Number(60.0), Some("area_1"))
            .unwrap();
        let meta = d.get("Hookshot.length").unwrap();
        assert_eq!(meta.source, DialSource::Scoped);
        assert_eq!(meta.scope.as_deref(), Some("area_1"));
    }

    #[test]
    fn set_refuses_a_curve_and_says_which_call_to_use() {
        // ⚠ Swapping a constant for a curve changes the kind, which `set` has no way to express.
        let mut d = dials();
        let err = d
            .set(
                "Hookshot.length",
                DialValue::Curve {
                    asset: "/Content/Curves/wear.cvcurve".into(),
                    row: "rate".into(),
                },
                None,
            )
            .unwrap_err();
        assert_eq!(
            err,
            DialError::WrongCall {
                id: "Hookshot.length".into(),
                use_instead: "setSource"
            }
        );
        assert!(err.to_string().contains("changes the dial's kind"));
    }

    #[test]
    fn set_source_swaps_a_constant_for_a_curve_at_runtime() {
        let mut d = dials();
        d.set_source(
            "Hookshot.length",
            DialValue::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            },
        )
        .unwrap();
        let meta = d.get("Hookshot.length").unwrap();
        assert_eq!(meta.kind, DialKind::Curve);
        assert!(meta.is_overridden());

        // ⚠ And reset puts the kind back too, or the dial would stay a curve forever.
        d.reset("Hookshot.length").unwrap();
        assert_eq!(d.get("Hookshot.length").unwrap().kind, DialKind::Number);
    }

    #[test]
    fn set_source_refuses_a_plain_value() {
        let mut d = dials();
        assert_eq!(
            d.set_source("Hookshot.length", DialValue::Number(1.0)),
            Err(DialError::WrongCall {
                id: "Hookshot.length".into(),
                use_instead: "set"
            })
        );
    }

    #[test]
    fn a_value_of_the_wrong_kind_is_refused_with_both_kinds_named() {
        let mut d = dials();
        let err = d
            .set("Hookshot.grade", DialValue::Number(1.0), None)
            .unwrap_err();
        assert_eq!(
            err,
            DialError::WrongKind {
                id: "Hookshot.grade".into(),
                expected: DialKind::Enum,
                given: DialKind::Number
            }
        );
    }

    #[test]
    fn a_value_outside_its_bounds_is_reported_rather_than_refused() {
        // ⚠ Content may have authored a default outside a range it later narrowed; refusing to report
        // the dial would hide the mistake rather than surface it.
        let mut d = Dials::new();
        d.declare(DialMeta::authored(
            "/Content/X",
            "n",
            DialValue::Number(500.0),
            DialBounds::number(0.0, 100.0),
        ));
        assert_eq!(d.len(), 1, "the dial is still listed");
        assert!(d.get("X.n").unwrap().is_out_of_bounds());
        assert_eq!(d.out_of_bounds().len(), 1);
    }

    #[test]
    fn an_adaptive_pair_is_not_a_range_and_the_two_are_different_kinds() {
        // ⚠ A widget drawing them as one slider would lie about which end is a preference.
        let soft = DialValue::Adaptive {
            soft_min: 3.0,
            hard_max: 5.0,
        };
        let hard = DialValue::Range { lo: 3.0, hi: 5.0 };
        assert_ne!(soft.kind(), hard.kind());
        assert_ne!(soft, hard);
    }

    #[test]
    fn all_six_kinds_cross_the_seam() {
        let values = [
            DialValue::Number(1.0),
            DialValue::Range { lo: 0.0, hi: 1.0 },
            DialValue::Adaptive {
                soft_min: 1.0,
                hard_max: 2.0,
            },
            DialValue::Enum("A".into()),
            DialValue::Curve {
                asset: "/c.cvcurve".into(),
                row: "r".into(),
            },
            DialValue::Table {
                asset: "/c.cvcurve".into(),
                axis: "depth".into(),
            },
        ];
        let kinds: Vec<DialKind> = values.iter().map(DialValue::kind).collect();
        assert_eq!(kinds, DialKind::ALL.to_vec());
        assert_eq!(DialKind::ALL.len(), 6);
    }

    #[test]
    fn only_curves_and_tables_are_sources() {
        assert!(!DialValue::Number(1.0).is_source());
        assert!(!DialValue::Enum("A".into()).is_source());
        assert!(DialValue::Curve {
            asset: "/c".into(),
            row: "r".into()
        }
        .is_source());
        assert!(DialValue::Table {
            asset: "/c".into(),
            axis: "depth".into()
        }
        .is_source());
    }

    #[test]
    fn the_revision_moves_whenever_the_recipe_does() {
        // ⚠ A changed dial is a different recipe, so a host can watch this to know the next generate
        // produces a different world.
        let mut d = dials();
        let before = d.revision();
        d.set("Hookshot.length", DialValue::Number(45.0), None)
            .unwrap();
        assert!(d.revision() > before);

        let after_set = d.revision();
        assert!(d.set("nope.nothing", DialValue::Number(1.0), None).is_err());
        assert_eq!(
            d.revision(),
            after_set,
            "a failed call must not move the recipe"
        );
    }

    #[test]
    fn an_unknown_dial_is_named_in_every_call() {
        let mut d = dials();
        for err in [
            d.get("ghost.x").unwrap_err(),
            d.set("ghost.x", DialValue::Number(1.0), None).unwrap_err(),
            d.set_source(
                "ghost.x",
                DialValue::Curve {
                    asset: "/c".into(),
                    row: "r".into(),
                },
            )
            .unwrap_err(),
            d.reset("ghost.x").unwrap_err(),
        ] {
            assert_eq!(
                err,
                DialError::Unknown {
                    id: "ghost.x".into()
                }
            );
        }
    }

    #[test]
    fn the_enum_bounds_carry_what_a_dropdown_offers() {
        let d = dials();
        let bounds = &d.get("Hookshot.grade").unwrap().bounds;
        assert_eq!(bounds.enum_path.as_deref(), Some("/Core/ItemClass"));
        assert_eq!(bounds.enum_values.len(), 2);
        assert!(bounds.admits(&DialValue::Enum("USEFUL".into())));
        assert!(!bounds.admits(&DialValue::Enum("NOT_A_VALUE".into())));
    }

    #[test]
    fn a_doc_survives_to_the_panel() {
        let d = dials();
        assert_eq!(
            d.get("Hookshot.length").unwrap().doc,
            "how far the rope reaches"
        );
    }
}
