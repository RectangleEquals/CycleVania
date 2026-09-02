//! **`Exclusion`** — what `forbids()` returns: a volume nothing may occupy, with declared escapes.
//!
//! # The negative half of an obligation, and why it is a value rather than a flag
//!
//! ⚠ **A forbidden thing is not a required thing with a sign bit.** The design says this about
//! [`Route`](crate::judge::Route) and it is the same statement here: *"X must not be here"* carries a
//! **reason** and a list of **exceptions**, and neither has anywhere to live on a boolean. A generator
//! that stored exclusions as flags would be able to refuse a placement and unable to say why — which
//! is the failure mode every report in this project exists to prevent.
//!
//! The negative half is expressible in exactly four places, and this is one of them: the `forbids()`
//! hook, [`Route`](crate::judge::Route)'s forbidden branch, `NegateRule`, and
//! [`SkipPolicy`](crate::gate::SkipPolicy). A developer wanting *"X must not be accessible without Y"*
//! writes a gate and marks it; a developer wanting *"nothing may stand here"* writes one of these.
//!
//! # `unless` is a class list, and deliberately not a tag query
//!
//! ⚠ [`Constraint::MountedOn`](crate::placement::Constraint::MountedOn) takes a `TagQuery` for the
//! opposite reason to the one that applies here. *"Any sconce"* written as four class ids silently
//! excludes the fifth someone adds later — a **permissive** filter should stay open. An escape from an
//! exclusion is the reverse: it is a **hole in a safety rule**, and a hole that widens when someone
//! adds a class is how a keep-out volume stops keeping anything out. So the escapes are named, one by
//! one, and adding a fifth is a decision somebody makes on purpose.

use crate::class::{Kind, ObjectBound};
use crate::collision::CollisionBody;
use crate::path::ClassPath;
use std::fmt;

/// A volume nothing may occupy, with declared escapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Exclusion {
    /// The excluded volume.
    pub volume: CollisionBody,
    /// Content permitted anyway.
    ///
    /// ⚠ Named classes rather than a query — see the module header. An escape is a hole in a safety
    /// rule, and holes do not get to widen on their own.
    pub unless: Vec<Kind<ObjectBound>>,
    /// Prose for the trace.
    ///
    /// ⚠ **Not optional.** An exclusion that refuses a placement without saying why produces a report
    /// line a developer cannot act on, and *"nothing is loose by accident"* becomes unverifiable.
    pub reason: String,
}

impl Exclusion {
    /// A volume nothing may occupy.
    pub fn new(volume: CollisionBody, reason: impl Into<String>) -> Self {
        Exclusion {
            volume,
            unless: Vec::new(),
            reason: reason.into(),
        }
    }

    /// Permit one class anyway.
    pub fn except(mut self, kind: Kind<ObjectBound>) -> Self {
        if !self.unless.contains(&kind) {
            self.unless.push(kind);
        }
        self
    }

    /// Is this class allowed inside the volume regardless?
    pub fn admits(&self, class: &ClassPath) -> bool {
        self.unless.iter().any(|k| k.path() == class)
    }

    /// Does this exclusion refuse that class?
    ///
    /// ⚠ **Refusal is the default and admission is the exception**, which is the direction that fails
    /// safe: a class nobody thought about is kept out, not let in.
    pub fn forbids(&self, class: &ClassPath) -> bool {
        !self.admits(class)
    }

    /// Is the volume empty — an exclusion that excludes nothing?
    ///
    /// ⚠ **Worth asking explicitly.** An empty volume is the shape a default-constructed
    /// `clearance()` has, and an exclusion built from one silently forbids nothing anywhere. Reporting
    /// it is the difference between a rule that is off and a rule that looks on.
    pub fn is_vacuous(&self) -> bool {
        self.volume.islands().is_empty()
    }
}

impl fmt::Display for Exclusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "excluded: {}", self.reason)?;
        if !self.unless.is_empty() {
            let names: Vec<&str> = self.unless.iter().map(|k| k.path().as_str()).collect();
            write!(f, " (unless {})", names.join(", "))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::{ClassRegistry, CoreClass};
    use crate::shape::Shape;
    use cv_determinism::Vec3;

    fn kind(path: &str) -> Kind<ObjectBound> {
        let mut r = ClassRegistry::with_core();
        r.register(
            ClassPath::new(path).unwrap(),
            ClassPath::new(ObjectBound::PATH).unwrap(),
        )
        .unwrap();
        Kind::new(&r, ClassPath::new(path).unwrap()).unwrap()
    }

    fn body() -> CollisionBody {
        CollisionBody::of(Shape::Cube {
            extents: Vec3::new(2.0, 2.0, 2.0),
            bevel: 0.0,
        })
    }

    #[test]
    fn refusal_is_the_default_and_admission_the_exception() {
        let e = Exclusion::new(body(), "the boss arena floor stays clear");
        assert!(e.forbids(&ClassPath::new("/Content/Prop/Crate").unwrap()));
        assert!(!e.admits(&ClassPath::new("/Content/Prop/Crate").unwrap()));
    }

    #[test]
    fn a_declared_escape_is_admitted_and_nothing_else_is() {
        let e = Exclusion::new(body(), "keep the landing pad clear")
            .except(kind("/Content/Prop/LandingLight"));
        assert!(e.admits(&ClassPath::new("/Content/Prop/LandingLight").unwrap()));
        assert!(
            e.forbids(&ClassPath::new("/Content/Prop/LandingBeacon").unwrap()),
            "an escape names one class; a similar name is not the same class"
        );
    }

    #[test]
    fn an_escape_is_not_added_twice() {
        let e = Exclusion::new(body(), "r")
            .except(kind("/Content/Prop/A"))
            .except(kind("/Content/Prop/A"));
        assert_eq!(e.unless.len(), 1);
    }

    #[test]
    fn an_empty_volume_reports_itself_as_vacuous() {
        // The rule that looks on and is off.
        let e = Exclusion::new(CollisionBody::empty(), "nothing here");
        assert!(e.is_vacuous());
        assert!(!Exclusion::new(body(), "r").is_vacuous());
    }

    #[test]
    fn the_reason_reaches_the_trace() {
        let e = Exclusion::new(body(), "the boss arena floor stays clear")
            .except(kind("/Content/Prop/LandingLight"));
        let s = e.to_string();
        assert!(s.contains("the boss arena floor stays clear"));
        assert!(s.contains("/Content/Prop/LandingLight"));
    }
}
