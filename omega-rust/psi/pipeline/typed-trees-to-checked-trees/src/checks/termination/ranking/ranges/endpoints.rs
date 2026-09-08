//! Invocation-fixed scalar and direct-field endpoints for declared bounds.

use super::super::patterns;
use super::super::write_preservation::prefix_preserves_path;
use super::Bounds;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

mod endpoint_input;
use endpoint_input::EndpointInput;

pub(super) fn pinned_expression_bounds(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<Bounds> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let (low, high) =
        validation::immutable_integer_expression_bounds(program, machine, state, expression)?;
    let mut inputs = Vec::new();
    let mut pending = vec![expression];
    while let Some(input) = pending.pop() {
        match program.expression_table.expression(input) {
            ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {}
            ExpressionNode::Binary(binary) => {
                // The bounds owner has already checked the selected operator,
                // operand carriers and interval containment for this whole tree.
                pending.extend([binary.left, binary.right]);
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                inputs.push(EndpointInput::resolve(program, state, input)?)
            }
            _ => return None,
        }
    }
    let frames = validation::CallFrameResolver::new(program)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    // Immutable storage is not enough: transition actuals may replace that
    // formal on every iteration, even with another value of the same type.
    for edge in patterns::edges_to_state(program, state, state.symbol) {
        for input in &inputs {
            let actual = *edge.arguments.get(input.argument_position)?;
            if !input.preserved_by(program, actual)
                || !prefix_preserves_path(
                    &frames,
                    machine,
                    &statements[..=edge.statement_ordinal],
                    &input.path(),
                )
            {
                return None;
            }
        }
    }
    Some(Bounds {
        low: i128::from(low),
        high: i128::from(high),
    })
}
