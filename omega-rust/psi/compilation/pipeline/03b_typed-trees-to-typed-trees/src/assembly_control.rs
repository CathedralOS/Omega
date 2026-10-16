//! Admission for the currently executable lexical assembly control domain.
//! Local catalog transfers are unconditional; finite paths preserve their
//! source frame. Cycles remain a missing checked-loop capability, not a finite
//! instruction sequence or an assumed invariant.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) fn admit(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if !program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Assembly(_)
                    )
                })
            {
                continue;
            }
            let order = program
                .statement_table
                .finite_execution_order(state.statement_nodes)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "invalid assembly control in `{}` state `{}`: {reason}",
                        machine.name, state.name,
                    ))]
                })?;
            if order.is_none() {
                return Err(vec![Diagnostic::error(format!(
                    "cyclic assembly label control in `{}` state `{}` requires checked loop invariants; this control domain is not yet implemented",
                    machine.name, state.name,
                ))]);
            }
        }
    }
    Ok(())
}
