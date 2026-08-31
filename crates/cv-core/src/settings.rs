//! Project settings — the knobs a host sets before generation, and the units everything else is
//! measured in.
//!
//! # `world_scale` is not a presentation setting
//!
//! **Every spatial quantity in generation is relative to it.** A `Span(0.0, 30.0)` on a grapple is
//! thirty *units*, and what a unit means is this number.
//!
//! That sounds like a detail and is not. The first implementation shipped a world with **0.11
//! similarity** to its reference; setting absolute scale correctly moved it **0.085 → 0.296** — a
//! bigger single win than every algorithmic improvement before it. A core written against implicit
//! units cannot be retrofitted with a scale factor later without re-testing every spatial number in
//! it, which is why this lands before anything spatial is built on top.
//!
//! The default of `1.0` (one unit = one metre) matches Rapier's `length_unit` and the Three.js
//! convention, so a host that never thinks about it gets the right answer.
//!
//! # Two settings are not optional in practice
//!
//! * **`player_profile.speed`** — every `TimeBudget` is a distance divided by it. A profile without a
//!   speed makes the whole time axis unusable.
//! * **`starting_grants`** — sphere 0 is what is accessible holding only these. Without them nothing
//!   can move and the first room of any game cannot generate.
//!
//! Both have defaults here so a minimal project runs, and [`Settings::validate`] reports the ones
//! that are merely *present* rather than *considered*.

use std::fmt;

/// How a resource behaves over a run. Declared by the project; drawn against by a [`PoolBudget`].
///
/// [`PoolBudget`]: crate::mechanic
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceDef {
    /// The name a `PoolBudget` names.
    pub name: String,
    /// Maximum an occupant may hold.
    pub capacity: f64,
    /// Whether it refills over time without a source.
    pub regenerates: bool,
    /// Whether an occupant starts at capacity.
    pub starts_full: bool,
}

/// The occupant the solver reasons about by default.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerProfile {
    /// Radius of the standing footprint, in units.
    pub footprint_radius: f64,
    /// Standing height, in units. The inner bound of a Space is floor volume extruded to this.
    pub standing_height: f64,
    /// Eye height, in units. Sightlines originate here, not at the feet.
    pub eye_height: f64,
    /// Ground speed, in units per second.
    ///
    /// ⚠ Not optional in practice: every `TimeBudget` is a distance divided by this.
    pub speed: f64,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        // A roughly human-sized occupant at world_scale = 1.0. Chosen so that a project which never
        // sets a profile still generates something a person could plausibly walk through, rather
        // than something geometrically valid and physically absurd.
        Self {
            footprint_radius: 0.35,
            standing_height: 1.9,
            eye_height: 1.7,
            speed: 5.0,
        }
    }
}

/// Structural density targets, per scope. Reported against, never enforced.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Density {
    /// Spaces per Area.
    pub spaces_per_area: Option<f64>,
    /// Spatial content per Space.
    pub spatials_per_space: Option<f64>,
    /// Traversal edges per Space.
    pub edges_per_space: Option<f64>,
}

/// A lock type the project declares, and what answers it.
///
/// The design law both halves of which should be checkable: *an instrument with no lock is a defect;
/// a lock with no instrument is a bug.* Declaring the vocabulary is what makes the second half
/// reportable at all.
#[derive(Clone, Debug, PartialEq)]
pub struct LockType {
    pub name: String,
    /// Content paths expected to answer it.
    pub answered_by: Vec<String>,
}

/// Everything a host sets before generation.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// **Units per metre.** Every spatial quantity in generation is relative to this.
    pub world_scale: f64,
    /// Occupancy sampling resolution, in units.
    ///
    /// ⚠ Must be at most `standing_height / 8`, or a standing occupant is fewer than eight samples
    /// tall and the floor detector starts missing ledges it should find.
    pub voxel_resolution: f64,
    pub player_profile: PlayerProfile,
    /// Token class paths an occupant holds at sphere 0.
    ///
    /// ⚠ Without at least one, nothing can move and the first room cannot generate.
    pub starting_grants: Vec<String>,
    /// Maximum slope, in degrees, that still counts as floor. Raised by a project with climbing.
    ///
    /// ⚠ Floor detection is **geometric**: nothing a `Surface` says removes a region. Lava is a floor
    /// with a restrictive `supports()` — present in the graph, gated — because if a surface could veto
    /// detection, hover boots could never create a route across it.
    pub max_floor_slope: f64,
    /// How far apart two floors may sit and still share an elevation band in the editor.
    pub elevation_band_tolerance: f64,
    /// How much of a route's slack the generator spends on obstacles. The rest is kept as margin.
    pub tension: f64,
    /// Scales declared thresholds. Reported, never a gate.
    pub difficulty: f64,
    /// The floor beneath a fidelity rung's own tolerance.
    ///
    /// A scalar comparison inside this band answers `AMBIGUOUS` rather than guessing.
    pub ambiguity_epsilon: f64,
    pub density: Density,
    pub resources: Vec<ResourceDef>,
    /// Names of declared world-state variables.
    pub state_vars: Vec<String>,
    pub lock_types: Vec<LockType>,
    /// The pair a run-back cost is measured between, as content paths.
    pub retraversal_target: Option<(String, String)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            world_scale: 1.0,
            voxel_resolution: 0.2,
            player_profile: PlayerProfile::default(),
            starting_grants: Vec::new(),
            max_floor_slope: 50.0,
            elevation_band_tolerance: 1.0,
            tension: 0.5,
            difficulty: 0.5,
            ambiguity_epsilon: 0.25,
            density: Density::default(),
            resources: Vec::new(),
            state_vars: Vec::new(),
            lock_types: Vec::new(),
            retraversal_target: None,
        }
    }
}

/// Something wrong with a project's settings, found before generation rather than during it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsIssue {
    pub setting: &'static str,
    pub detail: String,
    /// A `Warning` still generates; an `Error` does not.
    pub severity: Severity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for SettingsIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        write!(f, "{level}: `{}` — {}", self.setting, self.detail)
    }
}

impl Settings {
    /// Convert metres to generation units.
    ///
    /// Content authored in metres — a 30 m grapple, a 1.9 m occupant — passes through here exactly
    /// once, at the boundary. Anything further in is already in units.
    pub fn units(&self, metres: f64) -> f64 {
        metres * self.world_scale
    }

    /// Convert generation units back to metres, for a trace line or a report a human reads.
    pub fn metres(&self, units: f64) -> f64 {
        units / self.world_scale
    }

    /// Check the settings against each other.
    ///
    /// ⚠ The interesting ones are the *relationships*: a voxel resolution is only wrong relative to
    /// an occupant's height, and a starting grant list is only wrong relative to there being content
    /// to reach. A per-field range check would catch neither.
    pub fn validate(&self) -> Vec<SettingsIssue> {
        let mut out = Vec::new();
        let mut err = |setting, detail: String| {
            out.push(SettingsIssue {
                setting,
                detail,
                severity: Severity::Error,
            });
        };

        if !(self.world_scale.is_finite() && self.world_scale > 0.0) {
            err(
                "world_scale",
                format!("must be finite and positive, not {}", self.world_scale),
            );
        }
        if !(self.voxel_resolution.is_finite() && self.voxel_resolution > 0.0) {
            err(
                "voxel_resolution",
                format!("must be finite and positive, not {}", self.voxel_resolution),
            );
        }

        let p = &self.player_profile;
        for (name, v) in [
            ("player_profile.footprint_radius", p.footprint_radius),
            ("player_profile.standing_height", p.standing_height),
            ("player_profile.eye_height", p.eye_height),
            ("player_profile.speed", p.speed),
        ] {
            if !(v.is_finite() && v > 0.0) {
                err(name, format!("must be finite and positive, not {v}"));
            }
        }
        if p.eye_height > p.standing_height {
            err(
                "player_profile.eye_height",
                "cannot exceed standing_height".to_string(),
            );
        }

        // The relationship that actually matters: fewer than eight samples over a standing occupant
        // and the floor detector starts missing ledges a player could stand on.
        let ceiling = p.standing_height / 8.0;
        if self.voxel_resolution > ceiling {
            err(
                "voxel_resolution",
                format!(
                    "must be at most standing_height / 8 = {ceiling}, not {}",
                    self.voxel_resolution
                ),
            );
        }

        for (name, v) in [("tension", self.tension), ("difficulty", self.difficulty)] {
            if !(0.0..=1.0).contains(&v) {
                err(name, format!("must be within 0.0..=1.0, not {v}"));
            }
        }
        if !(self.max_floor_slope > 0.0 && self.max_floor_slope < 90.0) {
            err(
                "max_floor_slope",
                format!("must be within 0..90 degrees, not {}", self.max_floor_slope),
            );
        }
        if !(self.ambiguity_epsilon.is_finite() && self.ambiguity_epsilon >= 0.0) {
            err(
                "ambiguity_epsilon",
                format!(
                    "must be finite and non-negative, not {}",
                    self.ambiguity_epsilon
                ),
            );
        }

        for r in &self.resources {
            if r.name.is_empty() {
                err("resources", "a resource with no name".to_string());
            }
            if !(r.capacity.is_finite() && r.capacity > 0.0) {
                err(
                    "resources",
                    format!(
                        "`{}` needs a finite positive capacity, not {}",
                        r.name, r.capacity
                    ),
                );
            }
        }

        if self.starting_grants.is_empty() {
            out.push(SettingsIssue {
                setting: "starting_grants",
                detail: "sphere 0 is empty, so nothing can move and the first room cannot generate"
                    .to_string(),
                severity: Severity::Warning,
            });
        }

        out
    }

    /// Whether generation may proceed. Warnings do not stop it; errors do.
    pub fn is_generatable(&self) -> bool {
        !self
            .validate()
            .iter()
            .any(|i| i.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_generate_with_only_a_warning() {
        let s = Settings::default();
        assert!(s.is_generatable());
        let issues = s.validate();
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].setting, "starting_grants");
        assert_eq!(issues[0].severity, Severity::Warning);
    }

    #[test]
    fn scale_round_trips() {
        let s = Settings {
            world_scale: 11.0,
            ..Settings::default()
        };
        // The value that moved MP1 similarity 0.085 -> 0.296. A 30 m grapple is 330 units here.
        assert_eq!(s.units(30.0), 330.0);
        assert_eq!(s.metres(s.units(30.0)), 30.0);
    }

    #[test]
    fn voxel_resolution_is_checked_against_the_occupant() {
        // Valid on its own; wrong relative to a 1.9-unit occupant.
        let s = Settings {
            voxel_resolution: 0.5,
            ..Settings::default()
        };
        assert!(!s.is_generatable());
        assert!(s
            .validate()
            .iter()
            .any(|i| i.setting == "voxel_resolution" && i.detail.contains("standing_height / 8")));
    }

    #[test]
    fn eye_height_cannot_exceed_standing_height() {
        let s = Settings {
            player_profile: PlayerProfile {
                eye_height: 2.5,
                ..PlayerProfile::default()
            },
            ..Settings::default()
        };
        assert!(!s.is_generatable());
    }

    #[test]
    fn a_zero_speed_is_an_error_not_a_default() {
        // Every TimeBudget divides by this, so silently accepting zero would produce infinities
        // deep inside the solver rather than a message here.
        let s = Settings {
            player_profile: PlayerProfile {
                speed: 0.0,
                ..PlayerProfile::default()
            },
            ..Settings::default()
        };
        assert!(!s.is_generatable());
    }

    #[test]
    fn dials_are_bounded() {
        for (tension, difficulty) in [(1.5, 0.5), (0.5, -0.1)] {
            let s = Settings {
                tension,
                difficulty,
                ..Settings::default()
            };
            assert!(!s.is_generatable(), "{tension} {difficulty}");
        }
    }
}
