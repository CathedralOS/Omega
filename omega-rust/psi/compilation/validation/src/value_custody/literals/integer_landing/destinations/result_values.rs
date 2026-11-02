//! Landing destinations for result leaves, separate from dispatch inputs.
//! Validation has already admitted the value and its footprint. This visitor
//! preserves each element/arm boundary for fractional-origin diagnostics.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn visit_assignment_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
) {
    if let Some(indexed) = program.expression_table.indexed_node(target)
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
        && let Some(collection_type) = crate::value_custody::places::declared_place_type(
            program,
            machine,
            Some(state),
            indexed.collection,
        )
        && let Some((element_type, _)) =
            crate::value_custody::type_shapes::collection_element_and_length(
                program,
                collection_type,
            )
        && let Some(elements) = program.expression_table.array_literal_node(value)
    {
        for element in program.expression_table.expression_handles(*elements) {
            visit(element_type, *element);
        }
        return;
    }
    if let Some(destination) =
        crate::value_custody::places::declared_place_type_raw(program, machine, Some(state), target)
            .or_else(|| {
                crate::value_custody::places::declared_indexed_projection_type_raw(
                    program,
                    machine,
                    Some(state),
                    target,
                )
            })
    {
        visit(
            crate::value_custody::places::assignment_value_type(program, destination),
            value,
        );
    }
}

pub(super) fn visit_result_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
) {
    visit_result_values_inner(
        program,
        machine,
        state,
        destination,
        expression,
        visit,
        &mut Vec::new(),
    );
}

fn visit_result_values_inner(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
    active: &mut Vec<ExpressionHandle>,
) {
    use typed_trees::types::TypeReferenceNode;
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return;
    }
    if let Some(dispatch) = program.expression_table.match_node(expression) {
        // A typed arm owns the common result before any outer conversion;
        // wholly anonymous arms inherit their consumer's destination.
        let destination =
            match crate::value_custody::expression_types::expression_result_type_reference(
                program, machine, state, expression,
            ) {
                Some(retained) => retained,
                None if matches!(
                    program.type_reference_table.type_reference(
                        crate::value_custody::places::peel_declared_constraints(
                            program,
                            destination
                        ),
                    ),
                    TypeReferenceNode::FixedArray { .. }
                ) || crate::value_custody::literals::has_anonymous_numeric_results(
                    program, expression,
                ) =>
                {
                    destination
                }
                None => return,
            };
        active.push(expression);
        for arm in program.expression_table.match_arms(dispatch.arms) {
            visit_result_values_inner(
                program,
                machine,
                state,
                destination,
                arm.value,
                visit,
                active,
            );
        }
        active.pop();
    } else if let ExpressionNode::ArrayLiteral(elements) =
        program.expression_table.expression(expression)
        && let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(
                crate::value_custody::places::peel_declared_constraints(program, destination),
            )
    {
        active.push(expression);
        for element in program.expression_table.expression_handles(*elements) {
            visit_result_values_inner(
                program,
                machine,
                state,
                *element_type,
                *element,
                visit,
                active,
            );
        }
        active.pop();
    } else {
        visit(destination, expression);
    }
}
