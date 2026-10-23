//! Inspect a checked machine through the ordinary Psi production sequence.
//!
//! Inspection does not select an executable entry or install providers. It
//! selects a machine in the already checked source and produces its canonical
//! Terminal artifact, using the same stage entrances as product compilation.
//! Package acquisition and presentation remain with their respective callers.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionRequest, produce_terminal_artifact,
};

pub fn inspect_terminal(
    mut checked: CheckedCompilation,
    machine: &str,
) -> Result<terminal_codec::CanonicalTerminalArtifact, Vec<Diagnostic>> {
    let mut timings = std::mem::take(checked.timings_mut());
    produce_terminal_artifact(
        TerminalProductionRequest::new(
            &checked.execution().program,
            TerminalMachineSelection::Name(machine),
            checked.sources().exact_toolchain_sources(),
        ),
        &[],
        &mut timings,
    )
    .map(|produced| produced.into_artifact())
    .map_err(|error| vec![error.diagnostic()])
}
