//! Record every declared state's `let` locals for lowering
//! (`checked_trees::StateLocalFacts`).

use crate::flow::indexed_statements;
use checked_trees::{CheckedStateLocal, StateLocalFacts};

pub(crate) fn record_state_locals(program: &dyn validation::CheckingInput) -> StateLocalFacts {
    let mut states = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut locals = Vec::new();
            for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let Ok(statement_index) = u32::try_from(statement_index) else {
                    break;
                };
                locals.push(CheckedStateLocal {
                    statement_index,
                    symbol: local.symbol,
                    declared_type: local.type_reference,
                    type_is_inferred: local.type_is_inferred,
                    is_mutable: local.is_mutable,
                    // The locals `immutable_scalar_locals_before` counts.
                    is_scalar_value: !local.is_mutable
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some(),
                });
            }
            states.push((state.symbol, locals));
        }
    }
    StateLocalFacts::with_states(states)
}
