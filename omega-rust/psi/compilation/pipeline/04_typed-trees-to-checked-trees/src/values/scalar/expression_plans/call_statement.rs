//! A statement call's scalar arguments.

use super::{StatementPlanner, lower_call_arguments, retain_call_arguments};

pub(crate) fn plan(
    planner: &mut StatementPlanner<'_, '_>,
    call: &typed_trees::statement::TableCall,
) {
    if let Some(arguments) = lower_call_arguments(
        planner.program,
        planner.operators,
        planner.state,
        planner.statement_ordinal,
        0,
        &validation::CallSite::Statement(call),
        planner.scalar_parameters,
        planner.parameters,
        planner.parameter_types,
        planner.locals,
        planner.exact_integer_casts,
    ) {
        retain_call_arguments(
            arguments,
            planner.scalar_parameters,
            planner.locals,
            planner.expressions,
            planner.proof_terms,
            planner.source_bindings,
            planner.binding_symbols,
        );
    }
}
