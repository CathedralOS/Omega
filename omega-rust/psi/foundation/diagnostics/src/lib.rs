#![forbid(unsafe_code)]

//! Target-neutral diagnostics and phase-snapshot contracts for Psi.
//!
//! `Diagnostic` carries a severity, a message, the source span it refers to,
//! and — when a refusal stopped at a named private provision instead of
//! finding the source invalid — the `ExceededProvision` that outcome
//! publication reads in place of the wording; `PhaseSnapshot` records what one
//! phase observed for later reports; and `format_diagnostics` renders a batch
//! for the command line. Producers decide severity, wording and provision.
//! This crate only carries and prints them; bracketed names keep each
//! producer's naming policy in its callback.

mod diagnostic;
mod phase_snapshot;
mod reporter;

pub use diagnostic::{
    CoverageForm, Diagnostic, DiagnosticSeverity, ExceededProvision, ResolvedSourceLocation,
};
pub use phase_snapshot::PhaseSnapshot;
pub use reporter::{
    bracketed_display_names, comma_join_display, format_diagnostics, join_messages,
};

/// `Display` that delegates to `Debug`: the standard shape for
/// compiler-internal error enums whose rendered message is their variant data.
#[macro_export]
macro_rules! debug_display {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(self, formatter)
            }
        }
    };
}

/// `Display` that delegates to a `diagnostic` payload field.
#[macro_export]
macro_rules! diagnostic_display {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.diagnostic.fmt(formatter)
            }
        }
    };
}

/// String-newtype diagnostic convention: `Display` writes the carried message
/// and `Error` adds the marker impl. Single-line declaration for the common
/// `pub struct XDiagnostic(pub String)` shape.
#[macro_export]
macro_rules! string_diagnostic {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::error::Error for $type {}
    };
}

/// `Display` for the `<context> failed: {self:?}` error convention.
#[macro_export]
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

/// Single `error`-severity diagnostic with the producing site prefixed, for
/// call sites that surface exactly one refusal.
pub fn error_in_context(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
