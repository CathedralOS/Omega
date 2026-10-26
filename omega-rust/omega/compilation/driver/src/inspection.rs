//! Inspect a checked machine through the ordinary Psi production sequence.
//!
//! Inspection does not select an executable entry or install providers. It
//! selects a machine in the already checked source and produces its canonical
//! Terminal artifact, using the same stage entrances as product compilation.
//! Package acquisition and presentation remain with their respective callers.

use crate::CheckedCompilation;
use crate::observations::PsiStageTimings;
use diagnostics::Diagnostic;
use psi_compiler::{ProducedTerminalProgram, TerminalMachineSelection, TerminalProductionRequest};

pub fn inspect_terminal(
    mut checked: CheckedCompilation,
    machine: &str,
) -> Result<ProducedTerminalProgram, Vec<Diagnostic>> {
    let mut timings = std::mem::take(checked.timings_mut());
    TerminalProductionRequest::new(
        &checked.execution().program,
        TerminalMachineSelection::Name(machine),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut PsiStageTimings::new(&mut timings))
    .map_err(|error| vec![error.diagnostic()])
}
