//! A `let` initializer: a value call's scalar arguments, a view local's
//! subslice endpoints, or the initializer itself under its
//! `LocalInitializer` or `StorageInitializer` role. Every primitive local
//! then joins the state's local roster, which later statements read.
//! Only executable expressions belong here. Contract predicates can contain
//! proof-only equality vocabulary; accepting those as pure initializers would
//! suppress the ordinary computation builder that owns runtime evaluation.

use super::{
    CheckedLocatedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
    ExpressionNode, LoweredCallArguments, ScalarLocal, StatementPlanner, located_binding_symbols,
    lower_call_arguments, lower_direct_call_binding_arguments, lower_return_expression,
    lower_selected_operator_operands, retain_call_arguments, scalar_qualified_call_expression,
};

pub(super) fn plan(
    planner: StatementPlanner<'_, '_>,
    local: &typed_trees::statement::TableLocalData,
) {
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
        proof_terms,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    // A selected boundary operator's scalar operands carry
    // the same source custody as call arguments, whether
    // its result is scalar or structural.
    retain_call_arguments(
        LoweredCallArguments {
            scalar_arguments: lower_selected_operator_operands(
                program,
                operators,
                machine,
                state,
                statement_ordinal,
                local,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            ),
            proof_terms: Vec::new(),
        },
        scalar_parameters,
        locals,
        expressions,
        proof_terms,
        source_bindings,
        binding_symbols,
    );
    // Structural call results establish their own operation
    // place even when the local later lends mutable access.
    // Their scalar operands still need exact source rows,
    // found through the same cast chain an assignment's
    // call is.
    if (!local.is_mutable
        || program
            .primitive_type_reference(local.type_reference)
            .is_none())
        && let Some(expression) = scalar_qualified_call_expression(program, local.initial_value)
            .or_else(|| {
                matches!(
                    program.expression_table.expression(local.initial_value),
                    ExpressionNode::Call(_)
                )
                .then_some(local.initial_value)
            })
        && let ExpressionNode::Call(call) = program.expression_table.expression(expression)
        && let Some(arguments) = lower_call_arguments(
            program,
            operators,
            state,
            statement_ordinal,
            0,
            &crate::semantic::calls::CallSite::Expression { expression, call },
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    {
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
    // An element-spelling `let` (`let v: u8 = arr[a..b]`) binds the
    // same view an explicit `&[u8]` spelling does: when the
    // initializer's own result type is a borrowed slice view the local
    // plans like a view local — its endpoints mint under the subslice
    // roles and no scalar binding enters the roster.
    let Some(primitive_type) = program
        .primitive_type_reference(local.type_reference)
        .filter(|_| {
            local.is_mutable
                || validation::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    local.initial_value,
                )
                .and_then(|result| validation::slice_view_shell(program, result, &[]))
                .is_none()
        })
    else {
        if let Some(binding) = super::super::primitive_reference_read::local(program, local) {
            locals.push(binding);
        }
        return;
    };
    let binding_ordinal = u32::try_from(
        locals
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
                program,
                operators,
                state.symbol,
                statement_ordinal,
                binding_ordinal,
                local.initial_value,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )
        {
            retain_call_arguments(
                arguments,
                scalar_parameters,
                locals,
                expressions,
                proof_terms,
                source_bindings,
                binding_symbols,
            );
        } else if let Some(initializer) =
            crate::values::zeroed_scalar_local_initializer(program, local).or_else(|| {
                lower_return_expression(
                    program,
                    operators,
                    local.initial_value,
                    scalar_parameters,
                    parameters,
                    parameter_types,
                    locals,
                    primitive_type,
                    exact_integer_casts,
                )
            })
        {
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: local.symbol,
                state: state.symbol,
                statement_ordinal,
                role,
                expression: local.initial_value,
                symbols: binding_symbols
                    .insert_many(located_binding_symbols(scalar_parameters, locals)),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role,
                expression: initializer,
            });
        }
    }
    locals.push(ScalarLocal {
        is_mutable: local.is_mutable,
        symbol: local.symbol,
        name: local.name.as_str().to_owned(),
        primitive_type,
        arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
        is_reference: false,
    });
}
