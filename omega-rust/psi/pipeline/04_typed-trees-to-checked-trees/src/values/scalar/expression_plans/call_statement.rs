//! A statement call's scalar arguments.

use super::{StatementPlanner, lower_call_arguments, retain_call_arguments};

pub(super) fn plan(planner: StatementPlanner<'_, '_>, call: &typed_trees::statement::TableCall) {
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
        proof_terms,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    if let Some(arguments) = lower_call_arguments(
        program,
        operators,
        state,
        statement_ordinal,
        0,
        &crate::semantic::calls::CallSite::Statement(call),
        scalar_parameters,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    ) {
        retain_call_arguments(
            arguments,
            scalar_parameters,
            locals,
            expressions,
            proof_terms,
            source_bindings,
            binding_symbols,
        );
    }
}
