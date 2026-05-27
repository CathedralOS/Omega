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

        if !ranking::machine_has_proven_supported_decrease(program, machine) {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove decreases clause for terminating machine {}",
                machine_name(program, machine.symbol)
            )));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
