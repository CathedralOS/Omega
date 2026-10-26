//! An assignment: an atomic carrier's authored operands, each index of the
//! target place (`AssignmentIndex`) and the stored value (`AssignmentValue`).

use super::{
    CheckedScalarExpressionRole, ExpressionNode, StatementPlanner,
    assignment_target_primitive_type, lower_index_expression, lower_return_expression,
    retain_subslice_endpoints,
};

pub(crate) fn plan(
    planner: &mut StatementPlanner<'_, '_>,
    assignment: &typed_trees::statement::TableAssignment,
) {
    // An atomic carrier retains its authored operands,
    // never its arithmetic model (`atomic_operands.rs`).
    if let Some(operands) = super::super::atomic_operands::lower(
        planner.program,
        planner.operators,
        planner.machine,
        planner.state,
        assignment,
        planner.scalar_parameters,
        planner.parameters,
        planner.parameter_types,
        planner.locals,
        planner.exact_integer_casts,
    ) {
        retain_subslice_endpoints(
            operands,
            planner.state.symbol,
            planner.statement_ordinal,
            planner.scalar_parameters,
            planner.locals,
            planner.expressions,
            planner.source_bindings,
            planner.binding_symbols,
        );
        return;
    }
    // Every indexed step of the target keeps its own
    // selector row, keyed by its depth from the target.
    for (depth, index) in
        validation::assignment_target_selectors(planner.program, assignment.target)
            .into_iter()
            .enumerate()
    {
        let Ok(depth) = u32::try_from(depth) else {
            break;
        };
        let Some(expression) = lower_index_expression(
            planner.program,
            planner.operators,
            index,
            planner.scalar_parameters,
            planner.parameters,
            planner.parameter_types,
            planner.locals,
            planner.exact_integer_casts,
        ) else {
            continue;
        };
        planner.record_scalar_value_pair(
            CheckedScalarExpressionRole::AssignmentIndex { depth },
            symbols::SymbolHandle::invalid(),
            index,
            expression,
        );
    }
    // A call delivering its result to an assignment target
    // needs the same scalar-argument custody rows as one
    // delivering it to a local: the arguments are evaluated
    // and transferred identically, and where the result
    // lands does not change their source bindings. The
    // qualified-call spelling below stays for the cast
    // chains only ordinary integer-returning machines have.
    if let Some(expression) = super::value_call_expression(planner.program, assignment.value) {
        planner.retain_expression_call_arguments(expression);
    }
    // Retain selected RHS meaning at the statement. A later
    // executable consumer still owns its admitted store shape.
    let Some(target_type_reference) = validation::expression_type_reference_in_state(
        planner.program,
        planner.state.symbol,
        planner.statement_index,
        assignment.target,
    ) else {
        return;
    };
    let Some(target_type) =
        assignment_target_primitive_type(planner.program, target_type_reference)
    else {
        return;
    };
    let Some(expression) = lower_return_expression(
        planner.program,
        planner.operators,
        assignment.value,
        planner.scalar_parameters,
        planner.parameters,
        planner.parameter_types,
        planner.locals,
        target_type,
        planner.exact_integer_casts,
    ) else {
        return;
    };
    planner.record_scalar_value_pair(
        CheckedScalarExpressionRole::AssignmentValue,
        match planner
            .program
            .expression_table
            .expression(assignment.target)
        {
            ExpressionNode::Name(path) => path.symbol,
            _ => symbols::SymbolHandle::invalid(),
        },
        assignment.value,
        expression,
    );
}
