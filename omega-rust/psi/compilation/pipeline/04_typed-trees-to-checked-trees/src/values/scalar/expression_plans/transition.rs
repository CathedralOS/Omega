//! A transition: its guard (`Guard`), a returned arm value (`Return` or
//! `ContinuationReturn`), and each successor argument as a scalar
//! expression, a subslice endpoint or an erased proof term.

use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, StatementPlanner, TransitionGuardNode,
    TransitionTargetNode, lower_boolean_guard, lower_return_expression,
};

pub(crate) fn plan(
    planner: StatementPlanner<'_, '_>,
    transition: &typed_trees::statement::TableTransition,
) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        proof_only,
        machine,
        state,
        states,
        parameters,
        scalar_parameters,
        parameter_types,
        result_type,
        statement_ordinal,
        locals,
        expressions,
        proof_terms,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    if let TransitionGuardNode::When(authored_guard) = transition.guard
        && let Some(guard) = lower_boolean_guard(
            program,
            operators,
            authored_guard,
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    {
        super::record_scalar_value_pair(
            source_bindings,
            expressions,
            binding_symbols,
            scalar_parameters,
            locals,
            state.symbol,
            statement_ordinal,
            CheckedScalarExpressionRole::Guard,
            symbols::SymbolHandle::invalid(),
            authored_guard,
            CheckedScalarExpression::Boolean(Box::new(guard)),
        );
    }
    for (target, continuation) in [(transition.target, false), (transition.continuation, true)] {
        if !target.is_valid() {
            continue;
        }
        if transition.exit == typed_trees::statement::TransitionExit::Ordinary
            && let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
            && let Some(result_type) = result_type
        {
            let return_expression = lower_return_expression(
                program,
                operators,
                *expression,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                result_type,
                exact_integer_casts,
            );
            if return_expression.is_none()
                && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some()
            {
                // Temporary refusal imaging: pure-plan absence may legitimately
                // select the computation owner. Do not alter either decision.
                let declared_locals = program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                    .filter(|(index, _)| *index < statement_ordinal as usize)
                    .filter_map(|(index, statement)| {
                        statement.as_local_data().map(|local| {
                            (
                                index,
                                local.symbol,
                                local.type_reference,
                                program
                                    .type_reference_table
                                    .type_reference(local.type_reference),
                                local.initial_value,
                            )
                        })
                    })
                    .take(16)
                    .collect::<Vec<_>>();
                let declared_parameters = parameters
                    .iter()
                    .take(16)
                    .map(|parameter| {
                        (
                            parameter.symbol,
                            parameter.type_reference,
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "SCALAR_RETURN_PLAN_REFUSAL state={:?} statement={} continuation={} expression={:?} node={:?} result={:?} scalar_local_count={} scalar_locals={:?} declared_locals={:?} parameters={:?}",
                    state.symbol,
                    statement_ordinal,
                    continuation,
                    expression,
                    program.expression_table.expression(*expression),
                    result_type,
                    locals.len(),
                    &locals[..locals.len().min(16)],
                    declared_locals,
                    declared_parameters,
                );
            }
            let Some(return_expression) = return_expression else {
                continue;
            };
            let role = if continuation {
                CheckedScalarExpressionRole::ContinuationReturn
            } else {
                CheckedScalarExpressionRole::Return
            };
            super::record_scalar_value_pair(
                source_bindings,
                expressions,
                binding_symbols,
                scalar_parameters,
                locals,
                state.symbol,
                statement_ordinal,
                role,
                symbols::SymbolHandle::invalid(),
                *expression,
                return_expression,
            );
        }
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(target)
        else {
            continue;
        };
        let Some(target_state) =
            validation::activation_target_state_index(program, machine, path.symbol)
                .and_then(|target_index| states.get(target_index))
                .or_else(|| {
                    // Free names retain the machine head, while qualified names can
                    // retain its entry child. Both supply the first state's formals;
                    // foreign interior states remain invalid call targets.
                    validation::find_machine_head(program, path.symbol).map(|(_, entry)| entry)
                })
        else {
            continue;
        };
        let target_parameters = program.state_parameters(target_state).iter().enumerate();
        for (argument, (target_position, target_parameter)) in program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .zip(target_parameters)
        {
            let Ok(argument_ordinal) = u32::try_from(target_position) else {
                continue;
            };
            let Some(target_type) = (if target_parameter.relevance.is_erased() {
                program.erased_scalar_observation_type(target_parameter.type_reference)
            } else {
                program.primitive_type_reference(target_parameter.type_reference)
            }) else {
                // An erased contract-term formal records a
                // proof term under the same authored
                // argument ordinal the scalar lane uses.
                if target_parameter.relevance.is_erased()
                    && (proof_only.contract_term_carrier(program, target_parameter.type_reference)
                        || validation::mathematical_applications::is_source_evidence_carrier(
                            program,
                            target_parameter.type_reference,
                        ))
                {
                    if let Some(term) = crate::values::scalar::call_lowering::lower_proof_term(
                        program,
                        operators,
                        *argument,
                        target_parameter.type_reference,
                        scalar_parameters,
                        parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                        proof_only,
                    ) {
                        proof_terms.push(
                            checked_trees::CheckedLocatedProofTerm {
                                state: state.symbol,
                                statement_ordinal,
                                role: if continuation {
                                    checked_trees::CheckedProofTermRole::TransitionContinuationArgument {
                                        argument_ordinal,
                                    }
                                } else {
                                    checked_trees::CheckedProofTermRole::TransitionArgument {
                                        argument_ordinal,
                                    }
                                },
                                expression: *argument,
                                term,
                            },
                        );
                    }
                    // The erased argument owns no scalar
                    // expression: the term above is its
                    // whole contribution to the plan.
                    continue;
                }
                if continuation {
                    continue;
                }
                continue;
            };
            let Some(expression) = crate::values::scalar::call_lowering::lower_call_scalar_argument(
                target_parameter.relevance.is_erased(),
                state.symbol,
                statement_ordinal,
                program,
                operators,
                *argument,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                target_type,
                exact_integer_casts,
            ) else {
                continue;
            };
            let role = if continuation {
                CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
            } else {
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
            };
            super::record_scalar_value_pair(
                source_bindings,
                expressions,
                binding_symbols,
                scalar_parameters,
                locals,
                state.symbol,
                statement_ordinal,
                role,
                target_parameter.symbol,
                *argument,
                expression,
            );
        }
    }
}
