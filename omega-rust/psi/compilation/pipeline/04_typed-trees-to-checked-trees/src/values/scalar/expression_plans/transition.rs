//! A transition: its guard (`Guard`), a returned arm value (`Return` or
//! `ContinuationReturn`), and each successor argument as a scalar
//! expression, a subslice endpoint or an erased proof term.

use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, StatementPlanner, TransitionGuardNode,
    TransitionTargetNode, lower_boolean_guard, lower_return_expression,
};

pub(crate) fn plan(
    planner: &mut StatementPlanner<'_, '_>,
    transition: &typed_trees::statement::TableTransition,
) {
    if let TransitionGuardNode::When(authored_guard) = transition.guard
        && let Some(guard) = lower_boolean_guard(
            planner.program,
            planner.operators,
            authored_guard,
            planner.scalar_parameters,
            planner.parameters,
            planner.parameter_types,
            planner.locals,
            planner.exact_integer_casts,
        )
    {
        planner.record_scalar_value_pair(
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
                planner.program.statement_table.transition_target(target)
            && let Some(result_type) = planner.result_type
            && let Some(return_expression) = lower_return_expression(
                planner.program,
                planner.operators,
                *expression,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                result_type,
                planner.exact_integer_casts,
            )
        {
            let role = if continuation {
                CheckedScalarExpressionRole::ContinuationReturn
            } else {
                CheckedScalarExpressionRole::Return
            };
            planner.record_scalar_value_pair(
                role,
                symbols::SymbolHandle::invalid(),
                *expression,
                return_expression,
            );
        }
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = planner.program.statement_table.transition_target(target)
        else {
            continue;
        };
        let Some(target_state) = validation::activation_target_state_index(
            planner.program,
            planner.machine,
            path.symbol,
        )
        .and_then(|target_index| planner.states.get(target_index))
        .or_else(|| {
            // Free names retain the machine head, while qualified names can
            // retain its entry child. Both supply the first state's formals;
            // foreign interior states remain invalid call targets.
            validation::find_machine_head(planner.program, path.symbol).map(|(_, entry)| entry)
        }) else {
            continue;
        };
        let target_parameters = planner
            .program
            .state_parameters(target_state)
            .iter()
            .enumerate();
        for (argument, (target_position, target_parameter)) in planner
            .program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .zip(target_parameters)
        {
            let Ok(argument_ordinal) = u32::try_from(target_position) else {
                continue;
            };
            let Some(target_type) = (if target_parameter.relevance.is_erased() {
                planner
                    .program
                    .erased_scalar_observation_type(target_parameter.type_reference)
            } else {
                planner
                    .program
                    .primitive_type_reference(target_parameter.type_reference)
            }) else {
                // An erased contract-term formal records a
                // proof term under the same authored
                // argument ordinal the scalar lane uses.
                if target_parameter.relevance.is_erased()
                    && (planner
                        .proof_only
                        .contract_term_carrier(planner.program, target_parameter.type_reference)
                        || validation::mathematical_applications::is_source_evidence_carrier(
                            planner.program,
                            target_parameter.type_reference,
                        ))
                {
                    if let Some(term) = crate::values::scalar::call_lowering::lower_proof_term(
                        planner.program,
                        planner.operators,
                        *argument,
                        target_parameter.type_reference,
                        planner.scalar_parameters,
                        planner.parameters,
                        planner.parameter_types,
                        planner.locals,
                        planner.exact_integer_casts,
                        planner.proof_only,
                    ) {
                        planner.proof_terms.push(
                            checked_trees::CheckedLocatedProofTerm {
                                state: planner.state.symbol,
                                statement_ordinal: planner.statement_ordinal,
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
                planner.state.symbol,
                planner.statement_ordinal,
                planner.program,
                planner.operators,
                *argument,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                target_type,
                planner.exact_integer_casts,
            ) else {
                continue;
            };
            let role = if continuation {
                CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
            } else {
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
            };
            planner.record_scalar_value_pair(role, target_parameter.symbol, *argument, expression);
        }
    }
}
