//! Invocation-fixed scalar endpoints for the declared-bounds proof tier.

use super::super::patterns;
use super::super::write_preservation::prefix_preserves_path;
use super::{Bounds, same_parameter};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

pub(super) fn pinned_parameter_bounds(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<Bounds> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    let [spelling] = program.expression_table.name_path_members(name.members) else {
        return None;
    };
    let (ordinal, parameter) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find(|(_, parameter)| {
            name.symbol.is_valid()
                && name.head_symbol == name.symbol
                && name.symbol == parameter.symbol
                && *spelling == parameter.name
                && !parameter.is_mutable
                && !parameter.is_const
        })?;
    let (low, high) = validation::enforced_integer_type_bounds(program, parameter.type_reference)?;
    let frames = validation::CallFrameResolver::new(program)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    // Immutable storage is not enough: transition actuals may replace that
    // formal on every iteration, even with another value of the same type.
    for edge in patterns::edges_to_state(program, state, state.symbol) {
        let actual = *edge.arguments.get(ordinal)?;
        if !same_parameter(program, expression, actual)
            || !prefix_preserves_path(
                &frames,
                machine,
                &statements[..=edge.statement_ordinal],
                parameter.name.as_str(),
            )
        {
            return None;
        }
    }
    Some(Bounds {
        low: i128::from(low),
        high: i128::from(high),
    })
}
