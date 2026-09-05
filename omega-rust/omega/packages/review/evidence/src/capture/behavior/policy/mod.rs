//! Checked callable behavior projection into receipt-free policy.

mod crash;
mod flow;
mod mutation;
mod termination;

pub(crate) use crash::{crash, crash_routes};
pub(crate) use flow::{capability_flows, reachable_capability_flows};
pub(crate) use mutation::mutation;
pub(crate) use termination::{declared_termination, termination};

fn rejected(message: &str) -> Vec<psi_diagnostics::Diagnostic> {
    vec![psi_diagnostics::Diagnostic::error(format!(
        "callable behavior policy: {message}"
    ))]
}
