//! Compiler-owned projection of inert calling-contract components.

pub(crate) mod application;
pub(crate) mod callbacks;
mod opaque;
mod physical;

pub(crate) use application::project_checked_calling_policy;

use diagnostics::Diagnostic;

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "calling policy rejects {reason}"
    ))]
}
