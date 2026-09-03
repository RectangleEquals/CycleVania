//! **Asset loading** — the file half of the data resources, mesh import, and the table references
//! resolve through.
//!
//! ⚠ **The models already existed; this crate is the *files*.** `CurveTable`, `UnlockTable` and the
//! collision types are the core's, and a loader that redeclared them would be a second definition of
//! what a curve is.
#![forbid(unsafe_code)]

pub mod json;
pub mod mesh;
pub mod project;
pub mod resolve;
pub mod tables;

pub use mesh::{import, Bounds, Mesh, MeshError, MeshRef};
pub use project::{files_under, Descriptor, ProjectError};
pub use resolve::{digest_of, AssetId, AssetTable, ResolveError};
pub use tables::{load_curves, load_unlocks, LoadError, LoadedCurves};
