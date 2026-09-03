//! **The editor backend** — one service, one protocol, both deployment shapes.
//!
//! ⚠ **One frontend, two shapes: a browser tab and a desktop-packaged build.** Both talk to *this*
//! service over the same protocol, so every view is written once. The desktop shell does **not** get a
//! private in-process path — it opens a loopback connection to the identical backend a tablet on the
//! LAN reaches.
//!
//! That is a deliberate cost. A local call would be marginally faster; what it would buy is the outcome
//! where *"works locally"* and *"works remotely"* are two code paths and only one of them is exercised
//! daily.
//!
//! * [`protocol`] — the transport contract. ⚠ **Nothing in it names a deployment shape**, so there is
//!   no branch anywhere that could behave differently.
//! * [`auth`] — a pairing code, not an account. ⚠ **Loopback needs none**: a process that can reach
//!   `127.0.0.1` can already read the project off disk.
//! * [`service`] — the handler both shapes reach.

#![forbid(unsafe_code)]

pub mod auth;
pub mod connect;
pub mod dials_section;
pub mod palette;
pub mod protocol;
pub mod service;
pub mod state_view;
pub mod tables_view;
pub mod views;

pub use auth::{Auth, AuthError, Origin, Session};
pub use connect::{may_connect, widget_for, Dir, Pin, Refusal, Widget};
pub use dials_section::{DialBody, DialDraft, DialDraftError};
pub use palette::{Palette, PaletteNode, ProjectDial, Shape, Source, Utility};
pub use protocol::{Envelope, Request, Response, PROTOCOL_VERSION};
pub use service::Service;
pub use state_view::{Finding, State, StateGraph, Transition};
pub use tables_view::{
    DialRow, DialsView, FloorSlider, Rendering, TableFinding, UnlockRow, UnlockView,
};
pub use views::{browse, inspect, overrides, viewport, BrowseEntry, InspectorField, OverrideRow};

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
