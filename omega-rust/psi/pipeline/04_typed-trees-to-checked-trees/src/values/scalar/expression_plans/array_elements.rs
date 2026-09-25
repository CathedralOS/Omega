//! Array literal elements: an immutable local, a returned value or an
//! assignment whose destination is a closed primitive array plans each
//! element as a scalar expression under its `ArrayElement` role.

use super::*;

pub(super) fn plan(planner: StatementPlanner<'_, '_>, statement: &StatementNode) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        machine,
        state,
        parameters,
        scalar_parameters,
        parameter_types,
        statement_ordinal,
        locals,
        expressions,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    let array_destination = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            Some((local.initial_value, local.type_reference, local.symbol))
        }
        StatementNode::Expression(expression) => Some((
            *expression,
            state.return_type,
            symbols::SymbolHandle::invalid(),
        )),
        StatementNode::Assignment(assignment) => {
            validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
                .and_then(|declared| validation::closed_array_store_type(program, declared))
                .map(|expected| (assignment.value, expected, symbols::SymbolHandle::invalid()))
        }
        _ => None,
    };
    if let Some((expression, expected, destination)) = array_destination
        && let Some(elements) =
            validation::scalar_array_elements(program, machine.symbol, expression, expected)
    {
        for (element_index, (element, primitive_type)) in elements.elements.into_iter().enumerate()
        {
            let Ok(element_ordinal) = u32::try_from(element_index) else {
                break;
            };
            let Some(value) = lower_return_expression(
                program,
                operators,
                element,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                primitive_type,
                exact_integer_casts,
            ) else {
                continue;
            };
            let role = CheckedScalarExpressionRole::ArrayElement {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                element_ordinal,
            };
            source_bindings.append(CheckedScalarExpressionBindings {
                destination,
                state: state.symbol,
                statement_ordinal,
                role,
                expression: element,
                symbols: binding_symbols.insert_many(
                    scalar_parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .chain(
                            locals
                                .iter()
                                .filter(|local| !local.is_mutable)
                                .map(|local| local.symbol),
                        ),
                ),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role,
                expression: value,
            });
        }
    }
}
