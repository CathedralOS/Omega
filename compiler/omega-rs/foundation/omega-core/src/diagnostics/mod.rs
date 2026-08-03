//! Compatibility exports for diagnostics now owned by Psi.

pub mod diagnostic {
    pub use psi_diagnostics::{Diagnostic, DiagnosticSeverity};
}

pub mod phase_snapshot {
    pub use psi_diagnostics::PhaseSnapshot;
}

pub mod reporter {
    pub use psi_diagnostics::format_diagnostics;
}

pub use psi_diagnostics::{Diagnostic, DiagnosticSeverity, PhaseSnapshot, format_diagnostics};
