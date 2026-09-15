#![forbid(unsafe_code)]

//! Target-neutral diagnostics and phase-snapshot contracts for Psi.
//!
//! `Diagnostic` carries a severity, a message, and the source span it refers to;
//! `PhaseSnapshot` records what one phase observed for later reports; and
//! `format_diagnostics` renders a batch for the command line. Producers decide
//! severity and wording. This crate only carries and prints them.

mod diagnostic;
mod phase_snapshot;
mod reporter;

pub use diagnostic::{Diagnostic, DiagnosticSeverity};
pub use phase_snapshot::PhaseSnapshot;
pub use reporter::format_diagnostics;
