//! Range operands have one occurrence inventory for pure and computed values.
//! Planning records coordinates; the consumer owns selective evaluation.
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
    for (expression, site) in
        super::super::subslice_endpoints::sites(program, machine, state, statement_index, statement)
    {
        let endpoints = super::super::subslice_endpoints::subslice_endpoints(
            program,
            operators,
            expression,
            site,
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
