mod graph;
mod order;
mod ranking;

use crate::labels::machine_name;
use omega_core::diagnostics::Diagnostic;

pub(crate) fn check_machine_termination(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.terminates)
    {
        if !graph::machine_has_cycle(program, machine) {
            continue;
        }

        if machine.decreases.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "terminating machine {} contains a recursive cycle but has no decreases clause",
                machine_name(program, machine.symbol)
            )));
            continue;
        }

        match ranking::machine_decrease_outcome(program, machine) {
            ranking::DecreaseOutcome::Proven => {}
            ranking::DecreaseOutcome::Unproven => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove decreases clause for terminating machine {}",
                    machine_name(program, machine.symbol)
                )));
            }
            ranking::DecreaseOutcome::AmbiguousOrder { candidates } => {
                diagnostics.push(Diagnostic::error(format!(
                    "ambiguous decreases clause for terminating machine {}: the decreasing \
                     value has no single well-founded order; specify it explicitly with \
                     `decreases value -> Order` (candidates: {})",
                    machine_name(program, machine.symbol),
                    candidates.join(", ")
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
