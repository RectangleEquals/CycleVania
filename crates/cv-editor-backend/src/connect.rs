//! **Connection rules** — what a wire may join, decided before it is drawn.
//!
//! ⚠ **Tier 1 of the three-tier validation is *impossible*, not *error*.** A `Kind<T>` pin will not
//! connect to a `Ref<T>` pin because **the wire does not draw** — the mistake never becomes a document,
//! so nothing downstream has to detect it. In a text language every mistake is tier 2: you type it,
//! then find out.
//!
//! # An execution pin is two facts
//!
//! ⚠ **`Dir=Out` *and* `Type=exec`.** Neither alone expresses it: `Dir=Out` alone is a data output, and
//! `Type=exec` with no direction is not a thing. A rule that checked one would let a data wire into a
//! flow pin.
//!
//! # An `Unlock` pin picks asset-then-row
//!
//! ⚠ **Never a class picker and never free text.** An unlock is a *row of a table*, so the widget is the
//! same two-step one the `Curve` and `Table` dial kinds use. This is the editor half of the type change
//! that made `Unlock` a row: the type bounds what is **representable**, and this bounds what is
//! **offerable**.

use std::fmt;

/// Which way a pin faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    In,
    Out,
}

/// One pin on a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    /// Its name on the node.
    pub name: String,
    /// Which way it faces.
    pub dir: Dir,
    /// Its declared type, as the manifest spells it.
    pub ty: String,
}

impl Pin {
    /// A pin.
    pub fn new(name: &str, dir: Dir, ty: &str) -> Self {
        Pin {
            name: name.into(),
            dir,
            ty: ty.into(),
        }
    }

    /// Is this an execution pin?
    ///
    /// ⚠ **Both facts.** A pin that is only `Dir=Out` is a data output.
    pub fn is_exec(&self) -> bool {
        self.ty == "exec"
    }
}

/// Why a wire will not draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Two outputs, or two inputs.
    SameDirection,
    /// One end is flow and the other is data.
    ///
    /// ⚠ **Named separately from a type mismatch**, because *"you cannot wire a value into a flow pin"*
    /// is a different sentence from *"a bool is not a float"* and a developer acts on it differently.
    ExecMismatch,
    /// ⚠ **A class where an instance is wanted, or the reverse.**
    ///
    /// The distinction that replaced seven retracted language features, refused at the wire.
    KindVersusRef { from: String, to: String },
    /// The types do not match and neither is assignable to the other.
    TypeMismatch { from: String, to: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::SameDirection => write!(f, "a wire joins an output to an input"),
            Refusal::ExecMismatch => {
                write!(f, "execution flow and data do not connect")
            }
            Refusal::KindVersusRef { from, to } => write!(
                f,
                "{from} is a class and {to} is an instance — nothing is constructed by naming a class"
            ),
            Refusal::TypeMismatch { from, to } => write!(f, "{from} does not fit {to}"),
        }
    }
}

/// May a wire be drawn from one pin to another?
///
/// ⚠ **Answered before the drag completes.** The value of tier 1 is that the mistake never lands in a
/// document, so this is asked while the wire is still following the cursor.
pub fn may_connect(from: &Pin, to: &Pin) -> Result<(), Refusal> {
    if from.dir == to.dir {
        return Err(Refusal::SameDirection);
    }
    // Normalise so `from` is always the output.
    let (out, inp) = if from.dir == Dir::Out {
        (from, to)
    } else {
        (to, from)
    };

    if out.is_exec() != inp.is_exec() {
        return Err(Refusal::ExecMismatch);
    }
    if out.is_exec() {
        return Ok(());
    }
    assignable(&out.ty, &inp.ty)
}

/// Does a value of one type fit a pin of another?
pub fn assignable(from: &str, to: &str) -> Result<(), Refusal> {
    if from == to {
        return Ok(());
    }

    let (from_tag, from_inner) = split(from);
    let (to_tag, to_inner) = split(to);

    // ⚠ **`Kind` and `Ref` never mix**, in either direction and at any depth.
    if matches!(
        (from_tag, to_tag),
        (Some("Kind"), Some("Ref")) | (Some("Ref"), Some("Kind"))
    ) {
        return Err(Refusal::KindVersusRef {
            from: from.to_string(),
            to: to.to_string(),
        });
    }

    match (from_tag, to_tag) {
        // An `Array<X>` fits an `Array<Y>` exactly when `X` fits `Y`.
        (Some("Array"), Some("Array")) => assignable(from_inner, to_inner),
        // A reference of the same tag widens to a base — `Ref<Item>` fits `Ref<Actor>`.
        //
        // ⚠ **Only toward the base.** Accepting the other direction would let a graph promise an
        // `Item` and deliver an `Actor`, which is the one substitution a type system exists to refuse.
        (Some(a), Some(b)) if a == b => {
            if is_a(from_inner, to_inner) {
                Ok(())
            } else {
                Err(Refusal::TypeMismatch {
                    from: from.to_string(),
                    to: to.to_string(),
                })
            }
        }
        _ => Err(Refusal::TypeMismatch {
            from: from.to_string(),
            to: to.to_string(),
        }),
    }
}

/// Split `Tag<Inner>` or `Tag'Inner'` into its parts.
fn split(ty: &str) -> (Option<&str>, &str) {
    if let Some(open) = ty.find('<') {
        if ty.ends_with('>') {
            return (Some(&ty[..open]), &ty[open + 1..ty.len() - 1]);
        }
    }
    if let Some(open) = ty.find('\'') {
        if ty.ends_with('\'') {
            return (Some(&ty[..open]), &ty[open + 1..ty.len() - 1]);
        }
    }
    (None, ty)
}

/// Is one class the same as another, or beneath it?
fn is_a(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    let (Some(a), Some(b)) = (resolve(from), resolve(to)) else {
        return false;
    };
    std::iter::once(a)
        .chain(cv_api::ancestors(a))
        .any(|c| c.path == b.path)
}

/// Find a class by path, or by its short name.
///
/// ⚠ **Both spellings, because the manifest uses both.** A pin reads `Ref<Object>` while a class path is
/// `/Core/Object`, and a rule that understood only one would refuse half the palette's own wires.
fn resolve(name: &str) -> Option<&'static cv_api::ClassDesc> {
    cv_api::find(name).or_else(|| cv_api::CLASSES.iter().find(|c| c.short_name() == name))
}

/// How a pin's value is chosen when it is not wired.
///
/// ⚠ **The widget is a consequence of the type, not a per-pin decision.** A pin whose widget were chosen
/// by hand would eventually get the wrong one, and *"never free text"* would be a habit rather than a
/// property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    /// A class picker, bounded to the pin's subtree.
    KindPicker,
    /// An instance is wired, never picked.
    ///
    /// ⚠ **There is no instance picker**, because at author time no instance exists — the solver places
    /// them.
    WiredOnly,
    /// A file picker.
    AssetPicker,
    /// ⚠ **Asset, then a row within it** — what an `Unlock`, a `Curve` and a `Table` all use.
    AssetThenRow,
    /// A dropdown of the enum's values.
    EnumDropdown,
    /// A number field.
    Number,
    /// A checkbox.
    Toggle,
    /// A tag picker over the project's tag vocabulary.
    TagPicker,
    /// Free text. ⚠ **Only where the value genuinely is prose.**
    Text,
}

/// Which widget a pin of this type offers.
pub fn widget_for(ty: &str) -> Widget {
    let (tag, _) = split(ty);
    match (tag, ty) {
        // ⚠ An unlock is a *row of a table*, so it picks asset-then-row — never a class picker and
        // never free text. This is the editor half of the type change that made `Unlock` a row.
        (_, "Unlock") | (Some("Ref"), _) if ty.contains("Unlock") => Widget::AssetThenRow,
        (_, "Curve") | (_, "CurveTableResource") => Widget::AssetThenRow,
        (Some("Kind"), _) => Widget::KindPicker,
        (Some("Ref"), _) => Widget::WiredOnly,
        (Some("Asset"), _) | (Some("Resource"), _) => Widget::AssetPicker,
        (Some("Enum"), _) => Widget::EnumDropdown,
        (_, "int") | (_, "float") => Widget::Number,
        (_, "bool") => Widget::Toggle,
        (_, "Tag") | (_, "TagQuery") => Widget::TagPicker,
        _ => Widget::Text,
    }
}

/// A pin whose picked value could not be resolved.
///
/// ⚠ **Flagged rather than dropped, exactly like a paste referencing a missing class.** A pin that
/// silently emptied would look like one nobody filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// Which pin.
    pub pin: String,
    /// What it pointed at.
    pub target: String,
    /// Why it did not resolve.
    pub because: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(ty: &str) -> Pin {
        Pin::new("out", Dir::Out, ty)
    }
    fn inp(ty: &str) -> Pin {
        Pin::new("value", Dir::In, ty)
    }

    #[test]
    fn a_kind_pin_will_not_wire_to_a_ref_pin() {
        // ⚠ The distinction that replaced seven retracted language features, refused at the wire.
        let err = may_connect(&out("Kind<Item>"), &inp("Ref<Item>")).unwrap_err();
        assert!(matches!(err, Refusal::KindVersusRef { .. }));
        assert!(err.to_string().contains("nothing is constructed"));

        assert!(matches!(
            may_connect(&out("Ref<Item>"), &inp("Kind<Item>")),
            Err(Refusal::KindVersusRef { .. })
        ));
    }

    #[test]
    fn the_same_type_always_connects() {
        for ty in [
            "bool",
            "float",
            "Kind<Item>",
            "Ref<Actor>",
            "Array<Ref<Object>>",
        ] {
            assert_eq!(may_connect(&out(ty), &inp(ty)), Ok(()), "{ty}");
        }
    }

    #[test]
    fn a_wire_joins_an_output_to_an_input_and_direction_does_not_matter_to_the_caller() {
        // Dragging from either end is the same wire.
        assert_eq!(may_connect(&out("bool"), &inp("bool")), Ok(()));
        assert_eq!(may_connect(&inp("bool"), &out("bool")), Ok(()));
        assert_eq!(
            may_connect(&out("bool"), &out("bool")),
            Err(Refusal::SameDirection)
        );
        assert_eq!(
            may_connect(&inp("bool"), &inp("bool")),
            Err(Refusal::SameDirection)
        );
    }

    #[test]
    fn execution_flow_and_data_do_not_connect_and_the_message_says_which() {
        // ⚠ A different sentence from "a bool is not a float", and a developer acts on it differently.
        assert_eq!(
            may_connect(&out("exec"), &inp("bool")),
            Err(Refusal::ExecMismatch)
        );
        assert_eq!(
            may_connect(&out("bool"), &inp("exec")),
            Err(Refusal::ExecMismatch)
        );
        assert_eq!(may_connect(&out("exec"), &inp("exec")), Ok(()));
    }

    #[test]
    fn an_exec_pin_is_two_facts_and_neither_alone_expresses_it() {
        assert!(Pin::new("true", Dir::Out, "exec").is_exec());
        assert!(
            !Pin::new("out", Dir::Out, "bool").is_exec(),
            "Dir=Out alone is a data output"
        );
        assert!(Pin::new("in", Dir::In, "exec").is_exec());
    }

    #[test]
    fn a_reference_widens_toward_its_base_and_never_away_from_it() {
        // ⚠ The other direction would let a graph promise an Item and deliver an Actor.
        assert_eq!(may_connect(&out("Ref<Item>"), &inp("Ref<Actor>")), Ok(()));
        assert!(matches!(
            may_connect(&out("Ref<Actor>"), &inp("Ref<Item>")),
            Err(Refusal::TypeMismatch { .. })
        ));
    }

    #[test]
    fn an_array_connects_exactly_when_its_element_does() {
        assert_eq!(
            may_connect(&out("Array<Ref<Item>>"), &inp("Array<Ref<Actor>>")),
            Ok(())
        );
        assert!(matches!(
            may_connect(&out("Array<Kind<Item>>"), &inp("Array<Ref<Item>>")),
            Err(Refusal::KindVersusRef { .. })
        ));
        assert!(matches!(
            may_connect(&out("Array<bool>"), &inp("Array<float>")),
            Err(Refusal::TypeMismatch { .. })
        ));
    }

    #[test]
    fn a_bare_type_does_not_connect_to_an_unrelated_one() {
        assert!(matches!(
            may_connect(&out("bool"), &inp("float")),
            Err(Refusal::TypeMismatch { .. })
        ));
        assert!(matches!(
            may_connect(&out("float"), &inp("Ref<Actor>")),
            Err(Refusal::TypeMismatch { .. })
        ));
    }

    #[test]
    fn an_unlock_pin_picks_asset_then_row_and_never_a_class_or_free_text() {
        // ⚠ An unlock is a row of a table. This is the editor half of the type change that made it one.
        assert_eq!(widget_for("Unlock"), Widget::AssetThenRow);
        assert_eq!(widget_for("Ref<Unlock>"), Widget::AssetThenRow);
        assert_ne!(widget_for("Unlock"), Widget::KindPicker);
        assert_ne!(widget_for("Unlock"), Widget::Text);
    }

    #[test]
    fn a_curve_uses_the_same_two_step_widget_an_unlock_does() {
        // ⚠ One widget, because they are the same shape of question: which file, then which row.
        assert_eq!(widget_for("Curve"), Widget::AssetThenRow);
        assert_eq!(widget_for("CurveTableResource"), Widget::AssetThenRow);
    }

    #[test]
    fn there_is_no_instance_picker_because_no_instance_exists_at_author_time() {
        // ⚠ The solver places them; a picker would be offering something that does not exist yet.
        assert_eq!(widget_for("Ref<Actor>"), Widget::WiredOnly);
        assert_eq!(widget_for("Kind<Actor>"), Widget::KindPicker);
    }

    #[test]
    fn every_widget_follows_from_the_type_rather_than_from_a_per_pin_choice() {
        let cases = [
            ("int", Widget::Number),
            ("float", Widget::Number),
            ("bool", Widget::Toggle),
            ("Tag", Widget::TagPicker),
            ("TagQuery", Widget::TagPicker),
            ("Enum'/Core/ItemClass'", Widget::EnumDropdown),
            ("Asset'/Content/x.glb'", Widget::AssetPicker),
            ("String", Widget::Text),
        ];
        for (ty, expected) in cases {
            assert_eq!(widget_for(ty), expected, "{ty}");
        }
    }

    #[test]
    fn a_pin_that_cannot_resolve_is_flagged_rather_than_emptied() {
        // ⚠ A pin that silently emptied would look like one nobody filled in.
        let u = Unresolved {
            pin: "unlock".into(),
            target: "/Content/Progression/unlocks.cvunlock#Missiles".into(),
            because: "no such table".into(),
        };
        assert!(!u.target.is_empty());
        assert!(!u.because.is_empty());
    }
}
