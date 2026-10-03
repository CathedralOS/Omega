//! Retain endpoints for computed structural operands at expression occurrences.
//! Planning stores bindings only; selective evaluation remains with the consumer.
//! This is the sole CallArgument endpoint owner: the same semantic traversal
//! that captures flow calls supplies ordinals for statements, nested expressions,
//! and tail invocations. Result shape never determines operand custody.
use super::{
    PrimitiveType, StatementNode, StatementPlanner, lower_return_expression,
    retain_subslice_endpoints,
};

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
        statement_index,
        locals,
        expressions,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    for (call_ordinal, site) in crate::semantic::calls::collect_call_sites_in_statement(
        program,
        machine,
        state,
        statement_index,
    )
    .into_iter()
    .enumerate()
    {
        let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
            break;
        };
        for (argument, argument_ordinal) in structural_call_arguments(program, &site) {
            let endpoints = super::super::subslice_endpoints::subslice_endpoints(
                program,
                operators,
                argument,
                checked_trees::CheckedSubsliceSite::CallArgument {
                    call_ordinal,
                    argument_ordinal,
                },
                |endpoint| {
                    lower_return_expression(
                        program,
                        operators,
                        endpoint,
                        scalar_parameters,
                        parameters,
                        parameter_types,
                        locals,
                        PrimitiveType::U64,
                        exact_integer_casts,
                    )
                },
            );
            retain_subslice_endpoints(
                endpoints,
                state.symbol,
                statement_ordinal,
                scalar_parameters,
                locals,
                expressions,
                source_bindings,
                binding_symbols,
            );
        }
    }
    let table = &program.expression_table;
    let mut pending: Vec<_> = program
        .statement_table
        .expression_roots(statement)
        .into_iter()
        .map(|(_, root)| root)
        .filter(|root| root.is_valid())
        .collect();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !table.expression_is_valid(expression) || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let endpoints = super::super::subslice_endpoints::subslice_endpoints(
            program,
            operators,
            expression,
            checked_trees::CheckedSubsliceSite::Expression { expression },
            |endpoint| {
                lower_return_expression(
                    program,
                    operators,
                    endpoint,
                    scalar_parameters,
                    parameters,
                    parameter_types,
                    locals,
                    PrimitiveType::U64,
                    exact_integer_casts,
                )
            },
        );
        retain_subslice_endpoints(
            endpoints,
            state.symbol,
            statement_ordinal,
            scalar_parameters,
            locals,
            expressions,
            source_bindings,
            binding_symbols,
        );
        validation::expression_traversal::children(
            program,
            table.expression(expression),
            |child| {
                if child.is_valid() {
                    pending.push(child);
                }
            },
        );
    }
}

/// Structural formals have their own dense ordinal. Erased and scalar formals
/// consume their authored actual but no structural position; implicit receivers
/// consume neither an explicit actual nor its range coordinate.
fn structural_call_arguments(
    program: &typed_trees::TypedTrees,
    site: &crate::semantic::calls::CallSite<'_>,
) -> Vec<(typed_trees::expression::ExpressionHandle, u32)> {
    use crate::semantic::calls::CallSite;
    let target = match site {
        CallSite::Statement(call) => call.target_symbol,
        CallSite::Expression { call, .. } => crate::flow::call_target_symbol(program, call),
        CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    let Some(parameters) = crate::semantic::calls::call_target_parameters(program, target) else {
        return Vec::new();
    };
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, site);
    let explicit_self = arguments.len()
        > parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let explicit_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.is_self || explicit_self);
    if explicit_parameters.clone().count() != arguments.len() {
        return Vec::new();
    }
    let mut structural = Vec::new();
    for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
        let Some(erased) = crate::execution::terminal_unit::strips_erased_parameter(parameter)
        else {
            return Vec::new();
        };
        if erased
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        {
            continue;
        }
        let Ok(ordinal) = u32::try_from(structural.len()) else {
            return Vec::new();
        };
        structural.push((*argument, ordinal));
    }
    structural
}
