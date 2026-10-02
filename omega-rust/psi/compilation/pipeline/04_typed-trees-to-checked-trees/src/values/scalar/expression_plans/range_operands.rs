//! Retain endpoints for computed structural operands at expression occurrences.
//! Planning stores bindings only; selective evaluation remains with the consumer.
use super::{
    PrimitiveType, StatementNode, StatementPlanner, lower_return_expression,
    retain_subslice_endpoints,
};

pub(super) fn plan(planner: StatementPlanner<'_, '_>, statement: &StatementNode) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
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
