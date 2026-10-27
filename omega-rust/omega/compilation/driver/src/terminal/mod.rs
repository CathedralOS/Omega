//! Compiler packaging from a checked compilation to the canonical Terminal artifact.
//!
//! Start at `terminal_artifact.rs`. This compiler operation consumes one sealed checked
//! compilation and produces a Terminal application with its entry receipt and
//! closed provider companion. The native compiler owns application preparation:
//! it projects installed inputs, binds physical arrival custody and retains the
//! realization proposal. This coordinator never calls Omega stages directly.
//! Source production closes the Build provider companion against the emitted
//! demand catalog and returns it with each application. Native completion only
//! attaches those retained proposals; it never reopens typed provider selection.

mod behavior_exclusions;
mod terminal_artifact;

pub(crate) use terminal_artifact::{
    TerminalProductRequest, TerminalProductionView, produce_retained_terminal_artifacts,
};
