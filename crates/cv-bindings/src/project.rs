//! **The project descriptor** — load, validate, seed, generate.
//!
//! ```ts
//! const project = await cyclevania.load("./game.cvproj");
//! const world   = project.generate({ seed: "world-42" });
//! ```
//!
//! ⚠ **`load_from_file` is the whole host-facing surface for a cooked build.** A shipped game does not
//! parse schematics, walk a content root or run a compiler — it opens one file. Everything else in this
//! crate is what a *tool* reaches for.
//!
//! # The seed and the fingerprint are two different things
//!
//! ⚠ **The fingerprint is the recipe; the seed is the roll.** Two generates with the same fingerprint
//! and different seeds are two worlds from one design — which is the point of the whole system — and two
//! with different fingerprints are not comparable at all, however alike they look. A host that conflated
//! them would report *"same world"* for a run that shares only its dice.

use crate::dials::{DialError, Dials};
use std::fmt;

/// What a `generate` call is told.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerateOptions {
    /// The seed, as text.
    ///
    /// ⚠ **Text, not a number.** A seed a person types, shares in a bug report and reads back off a
    /// screen has to survive being written down; a `u64` printed in decimal does not survive a
    /// transcription error in a way anybody notices.
    pub seed: String,
}

impl GenerateOptions {
    /// Options with a seed.
    pub fn seeded(seed: impl Into<String>) -> Self {
        GenerateOptions { seed: seed.into() }
    }
}

/// Why a project call did not work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectError {
    /// The descriptor did not read.
    NotAProject { detail: String },
    /// Content did not validate.
    ///
    /// ⚠ **Every finding at once.** Stopping at the first makes fixing a project an *n*-pass job.
    Invalid { findings: Vec<String> },
    /// A dial call failed.
    Dial(DialError),
    /// `generate` was called before the project validated.
    ///
    /// ⚠ **Refused rather than run anyway.** Generating from content that did not validate produces a
    /// world whose faults are the *content's* and whose blame lands on the generator.
    NotValidated,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::NotAProject { detail } => write!(f, "not a project: {detail}"),
            ProjectError::Invalid { findings } => {
                write!(f, "{} problem(s): {}", findings.len(), findings.join("; "))
            }
            ProjectError::Dial(e) => write!(f, "{e}"),
            ProjectError::NotValidated => write!(
                f,
                "validate() before generate() — a world built from content that did not validate has \
                 the content's faults and the generator's blame"
            ),
        }
    }
}

impl From<DialError> for ProjectError {
    fn from(e: DialError) -> Self {
        ProjectError::Dial(e)
    }
}

impl std::error::Error for ProjectError {}

/// What a generate produced, as the host sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    /// The recipe this came from.
    pub fingerprint: u64,
    /// The seed it was rolled with.
    pub seed: String,
    /// How many scopes it has, as a smoke value until M21 wires the descriptor.
    pub scopes: usize,
}

/// A loaded project.
#[derive(Clone, Debug, Default)]
pub struct Project {
    /// Where it came from.
    pub path: String,
    /// Whether it is a cooked build rather than a content tree.
    ///
    /// ⚠ **Carried, and it changes nothing about dials.** They are inputs, not content — so a cooked
    /// project answers `list`, `get`, `set` and `setSource` exactly as a tool's does. Recording the
    /// flag is what lets a test assert that.
    pub cooked: bool,
    dials: Dials,
    validated: bool,
    findings: Vec<String>,
}

impl Project {
    /// A project loaded from a content tree.
    pub fn new(path: impl Into<String>) -> Self {
        Project {
            path: path.into(),
            ..Project::default()
        }
    }

    /// ⚠ **A cooked build: one file, and the whole host-facing surface.**
    pub fn load_from_file(path: impl Into<String>) -> Self {
        Project {
            path: path.into(),
            cooked: true,
            ..Project::default()
        }
    }

    /// The dial interface.
    pub fn dials(&self) -> &Dials {
        &self.dials
    }

    /// The dial interface, mutably.
    pub fn dials_mut(&mut self) -> &mut Dials {
        &mut self.dials
    }

    /// Record a validation finding, as loading content does.
    pub fn note(&mut self, finding: impl Into<String>) {
        self.findings.push(finding.into());
        self.validated = false;
    }

    /// Check the project.
    pub fn validate(&mut self) -> Result<(), ProjectError> {
        if !self.findings.is_empty() {
            return Err(ProjectError::Invalid {
                findings: self.findings.clone(),
            });
        }
        self.validated = true;
        Ok(())
    }

    /// Has it validated since the last change?
    pub fn is_validated(&self) -> bool {
        self.validated
    }

    /// **The recipe.**
    ///
    /// ⚠ **Dials are part of it and the seed is not.** A changed dial is a different recipe; a changed
    /// seed is the same recipe rolled again. That asymmetry is the whole reason both exist.
    pub fn fingerprint(&self) -> u64 {
        let mut acc = cv_determinism::hash::fnv1a_str(&self.path);
        for dial in self.dials.list() {
            acc = cv_determinism::hash::combine(acc, cv_determinism::hash::fnv1a_str(&dial.id));
            acc = cv_determinism::hash::combine(
                acc,
                cv_determinism::hash::fnv1a_str(&format!("{:?}", dial.effective)),
            );
        }
        acc
    }

    /// Generate a world.
    pub fn generate(&self, options: GenerateOptions) -> Result<World, ProjectError> {
        if !self.validated {
            return Err(ProjectError::NotValidated);
        }
        let fingerprint = self.fingerprint();
        let roll = cv_determinism::hash::combine(
            fingerprint,
            cv_determinism::hash::fnv1a_str(&options.seed),
        );
        Ok(World {
            fingerprint,
            seed: options.seed,
            // A stand-in until M21 wires the real descriptor: what matters here is that it moves with
            // both the recipe and the roll.
            scopes: (roll % 32) as usize + 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dials::{DialBounds, DialMeta, DialValue};

    fn project() -> Project {
        let mut p = Project::new("./game.cvproj");
        p.dials_mut().declare(DialMeta::authored(
            "/Content/Items/Hookshot",
            "length",
            DialValue::Number(30.0),
            DialBounds::number(8.0, 200.0),
        ));
        p.validate().unwrap();
        p
    }

    #[test]
    fn a_validated_project_generates() {
        let world = project()
            .generate(GenerateOptions::seeded("world-42"))
            .unwrap();
        assert_eq!(world.seed, "world-42");
        assert!(world.scopes > 0);
    }

    #[test]
    fn generate_before_validate_is_refused_rather_than_run() {
        // ⚠ A world built from content that did not validate has the content's faults and the
        // generator's blame.
        let p = Project::new("./game.cvproj");
        let err = p.generate(GenerateOptions::seeded("x")).unwrap_err();
        assert_eq!(err, ProjectError::NotValidated);
        assert!(err.to_string().contains("generator's blame"));
    }

    #[test]
    fn every_validation_finding_is_reported_at_once() {
        // ⚠ Stopping at the first makes fixing a project an n-pass job.
        let mut p = Project::new("./game.cvproj");
        p.note("a dangling asset");
        p.note("an unknown hook");
        let ProjectError::Invalid { findings } = p.validate().unwrap_err() else {
            panic!("expected findings");
        };
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn the_same_seed_and_the_same_dials_give_the_same_world() {
        let p = project();
        let a = p.generate(GenerateOptions::seeded("world-42")).unwrap();
        let b = p.generate(GenerateOptions::seeded("world-42")).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_dial_set_from_host_code_changes_the_generated_world() {
        // ⚠ M15's green condition, and the thing that proves dials are inputs.
        let mut p = project();
        let before = p.generate(GenerateOptions::seeded("world-42")).unwrap();

        p.dials_mut()
            .set("Hookshot.length", DialValue::Number(120.0), None)
            .unwrap();
        p.validate().unwrap();
        let after = p.generate(GenerateOptions::seeded("world-42")).unwrap();

        assert_ne!(
            before.fingerprint, after.fingerprint,
            "a changed dial is a different recipe"
        );
        assert_ne!(before, after, "and therefore a different world");
        assert_eq!(before.seed, after.seed, "with the same roll");
    }

    #[test]
    fn a_dial_set_in_a_cooked_build_changes_the_world_too() {
        // ⚠ The case that proves dials are inputs rather than content: nothing about cooking freezes
        // them, which is why this is the override channel a curve table points at.
        let mut cooked = Project::load_from_file("./game.cvpak");
        assert!(cooked.cooked);
        cooked.dials_mut().declare(DialMeta::authored(
            "/Content/Items/Hookshot",
            "length",
            DialValue::Number(30.0),
            DialBounds::number(8.0, 200.0),
        ));
        cooked.validate().unwrap();

        let before = cooked.generate(GenerateOptions::seeded("s")).unwrap();
        assert_eq!(
            cooked.dials().len(),
            1,
            "a cooked build still lists its dials"
        );

        cooked
            .dials_mut()
            .set("Hookshot.length", DialValue::Number(120.0), None)
            .unwrap();
        cooked.validate().unwrap();
        let after = cooked.generate(GenerateOptions::seeded("s")).unwrap();

        assert_ne!(before.fingerprint, after.fingerprint);
    }

    #[test]
    fn a_different_seed_is_the_same_recipe_rolled_again() {
        // ⚠ The asymmetry that is the whole reason both exist.
        let p = project();
        let a = p.generate(GenerateOptions::seeded("world-42")).unwrap();
        let b = p.generate(GenerateOptions::seeded("world-43")).unwrap();
        assert_eq!(
            a.fingerprint, b.fingerprint,
            "the recipe did not change; only the dice did"
        );
        assert_ne!(a.seed, b.seed);
    }

    #[test]
    fn the_seed_is_text_so_it_survives_being_written_down() {
        // ⚠ A seed a person types into a bug report and reads back off a screen.
        let p = project();
        let world = p
            .generate(GenerateOptions::seeded("the-flooded-wing"))
            .unwrap();
        assert_eq!(world.seed, "the-flooded-wing");
    }

    #[test]
    fn a_scoped_override_is_part_of_the_recipe_like_any_other() {
        let mut p = project();
        let before = p.fingerprint();
        p.dials_mut()
            .set("Hookshot.length", DialValue::Number(45.0), Some("area_1"))
            .unwrap();
        assert_ne!(p.fingerprint(), before);
    }

    #[test]
    fn resetting_a_dial_returns_the_project_to_its_original_recipe() {
        let mut p = project();
        let original = p.fingerprint();
        p.dials_mut()
            .set("Hookshot.length", DialValue::Number(45.0), None)
            .unwrap();
        assert_ne!(p.fingerprint(), original);
        p.dials_mut().reset("Hookshot.length").unwrap();
        assert_eq!(p.fingerprint(), original);
    }

    #[test]
    fn a_dial_error_reaches_the_host_as_a_project_error() {
        let mut p = project();
        let err: ProjectError = p
            .dials_mut()
            .set("ghost.x", DialValue::Number(1.0), None)
            .unwrap_err()
            .into();
        assert!(matches!(err, ProjectError::Dial(DialError::Unknown { .. })));
    }
}
