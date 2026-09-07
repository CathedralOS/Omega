//! Retype compiler-inferred temporaries from their exact selected call result.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

pub(super) fn refresh(program: &mut TypedTrees) {
    let selected_returns = program
        .machines()
        .iter()
        .filter(|machine| program.machine_type_parameters(machine).is_empty())
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| (state.symbol, state.return_type))
        .collect::<Vec<_>>();
    let bodies = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut updates = Vec::new();
    for body in bodies {
        for (offset, statement) in program.statement_table.statements(body).iter().enumerate() {
            let StatementNode::LocalData(local) = statement else {
                continue;
            };
            if !local.type_is_inferred {
                continue;
            }
            let ExpressionNode::Call(call) =
                program.expression_table.expression(local.initial_value)
            else {
                continue;
            };
            if call.target_symbol.is_valid()
                && let Some((_, return_type)) =
                    selected_returns.iter().find(|(state, return_type)| {
                        *state == call.target_symbol && return_type.is_valid()
                    })
            {
                // Retain the initializer occurrence and its evaluation point.
                // Only its inferred destination follows the selected instance.
                updates.push((body, offset, *return_type));
            }
        }
    }
    for (body, offset, return_type) in updates {
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        {
            local.type_reference = return_type;
        }
    }
}
