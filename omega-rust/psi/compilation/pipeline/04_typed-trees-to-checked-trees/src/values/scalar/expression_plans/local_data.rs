//! A `let` initializer: a value call's scalar arguments, a view local's
//! subslice endpoints, or the initializer itself under its
//! `LocalInitializer` or `StorageInitializer` role. Every primitive local
//! then joins the state's local roster, which later statements read.
//! Only executable expressions belong here. Contract predicates can contain
//! proof-only equality vocabulary; accepting those as pure initializers would
//! suppress the ordinary computation builder that owns runtime evaluation.

use super::{
    CheckedScalarExpressionRole, LoweredCallArguments, ScalarLocal, StatementPlanner,
    lower_direct_call_binding_arguments, lower_return_expression, lower_selected_operator_operands,
    retain_call_arguments,
};

pub(super) fn plan(
    planner: &mut StatementPlanner<'_, '_>,
    local: &typed_trees::statement::TableLocalData,
) {
    if let Some(carrier) = validation::atomic_once_carrier(planner.program, local.initial_value) {
        if let Some(operands) = super::super::atomic_operands::lower_carrier(
            planner.program,
            planner.operators,
            planner.machine,
            planner.state,
            carrier,
            planner.scalar_parameters,
            planner.parameters,
            planner.parameter_types,
            planner.locals,
            planner.exact_integer_casts,
        ) {
            for (source, role, value) in operands {
                planner.record_scalar_value_pair(
                    role,
                    symbols::SymbolHandle::invalid(),
                    source,
                    value,
                );
            }
        }
        return;
    }
    // A selected boundary operator's scalar operands carry
    // the same source custody as call arguments, whether
    // its result is scalar or structural.
    retain_call_arguments(
        LoweredCallArguments {
            scalar_arguments: lower_selected_operator_operands(
                planner.program,
                planner.operators,
                planner.machine,
                planner.state,
                planner.statement_ordinal,
                local,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                planner.exact_integer_casts,
            ),
            proof_terms: Vec::new(),
        },
        planner.scalar_parameters,
        planner.locals,
        planner.expressions,
        planner.proof_terms,
        planner.source_bindings,
        planner.binding_symbols,
    );
    // Structural call results establish their own operation
    // place even when the local later lends mutable access.
    // Their scalar operands still need exact source rows,
    // found through the same cast chain an assignment's
    // call is.
    if (!local.is_mutable
        || planner
            .program
            .primitive_type_reference(local.type_reference)
            .is_none())
        && let Some(expression) = super::value_call_expression(planner.program, local.initial_value)
    {
        planner.retain_expression_call_arguments(expression);
    }
    // An element-spelling `let` (`let v: u8 = arr[a..b]`) binds the
    // same view an explicit `&[u8]` spelling does: when the
    // initializer's own result type is a borrowed slice view the local
    // plans like a view local — its endpoints mint under the subslice
    // roles and no scalar binding enters the roster.
    let Some(primitive_type) = planner
        .program
        .primitive_type_reference(local.type_reference)
        .filter(|_| {
            local.is_mutable
                || validation::expression_result_type_reference(
                    planner.program,
                    planner.machine,
                    planner.state,
                    local.initial_value,
                )
                .and_then(|result| validation::slice_view_shell(planner.program, result, &[]))
                .is_none()
        })
    else {
        if let Some(binding) = super::super::primitive_reference_read::local(planner.program, local)
        {
            planner.locals.push(binding);
        }
        return;
    };
    let binding_ordinal = u32::try_from(
        planner
            .locals
            .iter()
            .filter(|local: &&ScalarLocal| local.is_value_binding())
            .count(),
    )
    .ok();
    if let Some(binding_ordinal) = binding_ordinal {
        let role = if local.is_mutable {
            CheckedScalarExpressionRole::StorageInitializer
        } else {
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
        };
        if !local.is_mutable
            && let Some(arguments) = lower_direct_call_binding_arguments(
                planner.program,
                planner.operators,
                planner.state.symbol,
                planner.statement_ordinal,
                binding_ordinal,
                local.initial_value,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                planner.exact_integer_casts,
            )
        {
            retain_call_arguments(
                arguments,
                planner.scalar_parameters,
                planner.locals,
                planner.expressions,
                planner.proof_terms,
                planner.source_bindings,
                planner.binding_symbols,
            );
        } else if let Some(initializer) =
            crate::values::zeroed_scalar_local_initializer(planner.program, local).or_else(|| {
                lower_return_expression(
                    planner.program,
                    planner.operators,
                    local.initial_value,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    primitive_type,
                    planner.exact_integer_casts,
                )
            })
        {
            planner.record_scalar_value_pair(role, local.symbol, local.initial_value, initializer);
        }
    }
    planner.locals.push(ScalarLocal::from_local_data(
        planner.program,
        local,
        primitive_type,
    ));
}
