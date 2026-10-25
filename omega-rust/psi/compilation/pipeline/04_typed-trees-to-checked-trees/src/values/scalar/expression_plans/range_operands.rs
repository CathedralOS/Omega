//! Range operands have one occurrence inventory for pure and computed values.
//! Planning records coordinates; the consumer owns selective evaluation.
use super::{StatementNode, StatementPlanner, lower_index_expression, retain_subslice_endpoints};

pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, statement: &StatementNode) {
    for (expression, site) in super::super::subslice_endpoints::sites(
        planner.program,
        planner.machine,
        planner.state,
        planner.statement_index,
        statement,
    ) {
        let endpoints = super::super::subslice_endpoints::subslice_endpoints(
            planner.program,
            planner.operators,
            expression,
            site,
            |endpoint| {
                lower_index_expression(
                    planner.program,
                    planner.operators,
                    endpoint,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    planner.exact_integer_casts,
                )
            },
        );
        retain_subslice_endpoints(
            endpoints,
            planner.state.symbol,
            planner.statement_ordinal,
            planner.scalar_parameters,
            planner.locals,
            planner.expressions,
            planner.source_bindings,
            planner.binding_symbols,
        );
    }
}
