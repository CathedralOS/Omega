#![forbid(unsafe_code)]

//! Target-neutral diagnostics and phase-snapshot contracts for Psi.

mod diagnostic;
mod phase_snapshot;
mod reporter;

pub use diagnostic::{Diagnostic, DiagnosticSeverity};
pub use phase_snapshot::PhaseSnapshot;
pub use reporter::format_diagnostics;
