//! A `let` initializer: a value call's scalar arguments, a view local's
//! subslice endpoints, or the initializer itself under its
//! `LocalInitializer` or `StorageInitializer` role. Every primitive local
//! then joins the state's local roster, which later statements read.
//! Only executable expressions belong here. Contract predicates can contain
//! proof-only equality vocabulary; accepting those as pure initializers would
//! suppress the ordinary computation builder that owns runtime evaluation.

use super::{
    ArithmeticDomain, CheckedLocatedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionRole, ExpressionNode, LoweredCallArguments, PrimitiveType, ScalarLocal,
    StatementPlanner, TypeReferenceNode, TypedTrees, lower_call_arguments,
    lower_direct_call_binding_arguments, lower_return_expression, lower_selected_operator_operands,
    retain_call_arguments, retain_subslice_endpoints, scalar_qualified_call_expression,
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
        // An immutable view local narrowed from another
        // view keeps its range endpoints under the same
        // subslice roles a call or transition argument
        // uses, at this statement's own binding site.
        if !local.is_mutable {
            let endpoints = super::super::subslice_endpoints::subslice_endpoints(
                program,
                operators,
                local.initial_value,
                checked_trees::CheckedSubsliceSite::LocalBinding,
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
        // A shared `&` local binds no value of its own: reads through it
        // re-evaluate the captured place at the use point, so the roster
        // entry carries that place for operands to lower.
        if let Some((place, primitive_type, arithmetic_domain)) = shared_loan_place(program, local)
        {
            locals.push(ScalarLocal {
                is_mutable: local.is_mutable,
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                primitive_type,
                arithmetic_domain,
                shared_loan_place: Some(place),
            });
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
                symbols: binding_symbols.insert_many(
                    scalar_parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .chain(
                            locals
                                .iter()
                                .filter(|local| local.is_value_binding())
                                .map(|local| local.symbol),
                        ),
                ),
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
        shared_loan_place: None,
    });
}

/// A place-loan local's captured place: the borrow target inside its
/// `&place` initializer, possibly under a recast, with the referee's
/// primitive carrier and domain for the operand reads that re-evaluate
/// the place. An immutable `&mut place` binding reads the same way: it
/// cannot be reseated, so it names that one place for its scope, exactly
/// the carrier `statement_sequence::local_data` plans as an exclusive
/// place loan and whose writes rejoin that place's store. A reseatable
/// binding, a view, a non-scalar or address-sized referee keeps no scalar
/// operand.
pub(crate) fn shared_loan_place(
    program: &TypedTrees,
    local: &typed_trees::statement::TableLocalData,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    PrimitiveType,
    ArithmeticDomain,
)> {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    let readable_loan = match access {
        language_semantics::ReferenceAccess::Shared => true,
        language_semantics::ReferenceAccess::Mutable => !local.is_mutable,
        // A write-only loan grants no observation of old contents.
        language_semantics::ReferenceAccess::WriteOnly => false,
    };
    if !readable_loan {
        return None;
    }
    let primitive_type = program.primitive_type_reference(*referee)?;
    if matches!(primitive_type, PrimitiveType::Addr) {
        return None;
    }
    let mut initializer = local.initial_value;
    let target = loop {
        match program.expression_table.expression(initializer) {
            ExpressionNode::Borrow(borrow) if borrow.access == *access => {
                break borrow.target;
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                initializer = cast.value;
            }
            _ => return None,
        }
    };
    Some((
        target,
        primitive_type,
        program.arithmetic_domain_for_type_reference(*referee),
    ))
}
