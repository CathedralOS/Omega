#![forbid(unsafe_code)]

//! Target-neutral diagnostics and phase-snapshot contracts for Psi.
//!
//! `Diagnostic` carries a severity, a message, the source span it refers to,
//! and — when a refusal stopped at a named private provision instead of
//! finding the source invalid — the `ExceededProvision` that outcome
//! publication reads in place of the wording; `PhaseSnapshot` records what one
//! phase observed for later reports; and `format_diagnostics` renders a batch
//! for the command line. Producers decide severity, wording and provision.
//! This crate only carries and prints them.

mod diagnostic;
mod phase_snapshot;
mod reporter;

pub use diagnostic::{CoverageForm, Diagnostic, DiagnosticSeverity, ExceededProvision};
pub use phase_snapshot::PhaseSnapshot;
pub use reporter::format_diagnostics;
