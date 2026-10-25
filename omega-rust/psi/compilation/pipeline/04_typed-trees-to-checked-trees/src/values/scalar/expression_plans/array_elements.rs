//! Array literal elements: an immutable local, a returned value or an
//! assignment whose destination is a closed primitive array plans each
//! element as a scalar expression under its `ArrayElement` role.

use super::{
    CheckedScalarExpressionRole, StatementNode, StatementPlanner, lower_return_expression,
};

pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, statement: &StatementNode) {
    let array_destination = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            Some((local.initial_value, local.type_reference, local.symbol))
        }
        StatementNode::Expression(expression) => Some((
            *expression,
            planner.state.return_type,
            symbols::SymbolHandle::invalid(),
        )),
        StatementNode::Assignment(assignment) => validation::assignment_stored_value_type(
            planner.program,
            planner.machine,
            planner.state,
            assignment,
        )
        .and_then(|declared| validation::closed_array_store_type(planner.program, declared))
        .map(|expected| (assignment.value, expected, symbols::SymbolHandle::invalid())),
        _ => None,
    };
    if let Some((expression, expected, destination)) = array_destination
        && let Some(elements) = validation::scalar_array_elements(
            planner.program,
            planner.machine.symbol,
            expression,
            expected,
        )
    {
        for (element_index, (element, primitive_type)) in elements.elements.into_iter().enumerate()
        {
            let Ok(element_ordinal) = u32::try_from(element_index) else {
                break;
            };
            let Some(value) = lower_return_expression(
                planner.program,
                planner.operators,
                element,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                primitive_type,
                planner.exact_integer_casts,
            ) else {
                continue;
            };
            let role = CheckedScalarExpressionRole::ArrayElement {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                element_ordinal,
            };
            planner.record_scalar_value_pair(role, destination, element, value);
        }
    }
}
