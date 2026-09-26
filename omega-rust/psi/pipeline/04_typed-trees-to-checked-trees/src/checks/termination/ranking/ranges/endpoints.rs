//! Invocation-fixed scalar and direct-field endpoints for declared bounds.

use super::super::patterns;
use super::super::write_preservation::prefix_preserves_path;
use super::Bounds;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
use symbols::SymbolHandle;

mod endpoint_input;
use endpoint_input::EndpointInput;

pub(super) fn pinned_expression_bounds(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    call_frames: Option<&crate::validation::CallFrameResolver<'_>>,
) -> Option<Bounds> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let (low, high) =
        crate::validation::immutable_integer_expression_bounds(program, machine, state, expression)
            .or_else(|| EndpointInput::declared_member_bounds(program, state, expression))?;
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
    let mut owned_frames = None;
    let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    // Immutable storage is not enough: transition actuals may replace that
    // formal on every iteration, even with another value of the same type.
    for edge in patterns::edges_to_state(program, state, state.symbol) {
        let locals: Vec<(SymbolHandle, ExpressionHandle)> = statements[..=edge.statement_ordinal]
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if !local.is_mutable => {
                    Some((local.symbol, local.initial_value))
                }
                _ => None,
            })
            .collect();
        for input in &inputs {
            let actual = *edge.arguments.get(input.argument_position)?;
            if !input.preserved_by(program, actual, &locals)
                || !prefix_preserves_path(
                    frames,
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
