//! cv-manifest — the tier-1 API manifest: the single hand-authored declaration of the core surface,
//! and the validators that keep it legal.
//!
//! **M00: freeze tier-1.** `manifest/tier1.toml` is the only place a tier-1 signature is written by
//! hand. Everything else — the Rust trait skeletons, the TypeScript declarations, the editor's node
//! palette, and the API reference document — is *generated* from it (M01). Editing any of those
//! directly is a bug, because the generator will overwrite it.
//!
//! * [`model`] — [`Manifest`], [`Class`], [`Field`], [`Method`]: the shape of a declaration.
//! * [`parse`] — a **strict subset** of TOML. Deliberately hand-written and deliberately narrow.
//! * [`validate`] — the binding constraints the generator refuses to build through.
//!
//! # Why a hand-written parser
//!
//! The manifest is a closed loop: this crate reads it and M01's generator writes it, so no third
//! party ever hands us a document. A general TOML implementation would accept far more than the
//! schema allows and turn a typo into a silently-wrong manifest. This parser accepts exactly the
//! constructs the schema uses and **errors on everything else**, which makes a malformed manifest a
//! build failure rather than a subtly wrong palette. The same reasoning shapes CVB's parser.
//!
//! # `field` versus `method` is semantic
//!
//! A plain read of something already known is a [`Field`] and becomes a pure get node with no exec
//! pins; anything that takes an argument, computes, or mutates is a [`Method`] and becomes a call
//! node with them. **The signal a method sends is cost.** A member tagged wrongly here ships a
//! wrong-shaped node into every generated palette, which is why the distinction is checked rather
//! than trusted.

pub mod model;
pub mod parse;
pub mod validate;

pub use model::{Class, Field, Kind, Manifest, Method, Param, Status, Value};
pub use parse::{parse, ParseError};
pub use validate::{validate, Violation};

/// The manifest as committed, for tests and for the generator.
pub const DEFAULT_PATH: &str = "manifest/tier1.toml";
