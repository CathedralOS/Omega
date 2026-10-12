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
        statement_index,
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
            && validation::current_entry_tail_call(
                program,
                machine,
                state,
                statement_index,
                *expression,
            )
            .is_none()
            && let Some(result_type) = result_type
            && let Some(return_expression) = lower_return_expression(
                program,
                operators,
                *expression,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                result_type,
                exact_integer_casts,
            )
        {
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
        let (target_state, arguments) = match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let Some(target_state) =
                    crate::checks::termination::named_transition_target_state_index(
                        program,
                        machine,
                        path.symbol,
                    )
                    .and_then(|index| states.get(index))
                    .or_else(|| {
                        validation::find_machine_head(program, path.symbol).map(|(_, entry)| entry)
                    })
                else {
                    continue;
                };
                (
                    target_state,
                    program.statement_table.expression_handles(*arguments),
                )
            }
            TransitionTargetNode::Value(expression) => {
                let Some(call) = validation::current_entry_tail_call(
                    program,
                    machine,
                    state,
                    statement_index,
                    *expression,
                ) else {
                    continue;
                };
                let Some(entry) = states.first() else {
                    continue;
                };
                (
                    entry,
                    program.expression_table.expression_handles(call.arguments),
                )
            }
            _ => continue,
        };
        arguments_plan(
            StatementPlanner {
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
                statement_index,
                statement_ordinal,
                locals: &mut *locals,
                expressions: &mut *expressions,
                proof_terms: &mut *proof_terms,
                source_bindings: &mut *source_bindings,
                binding_symbols: &mut *binding_symbols,
            },
            target_state,
            arguments,
            continuation,
        );
    }
}

/// The same successor namespace consumes Named state operands and the exact
/// whole-call operands preserved for ordinary source application admission.
pub(super) fn arguments_plan(
    planner: StatementPlanner<'_, '_>,
    target_state: &typed_trees::state::State,
    arguments: &[typed_trees::expression::ExpressionHandle],
    continuation: bool,
) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        proof_only,
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
    let target_parameters = program.state_parameters(target_state).iter().enumerate();
    for (argument, (target_position, target_parameter)) in arguments.iter().zip(target_parameters) {
        let Ok(argument_ordinal) = u32::try_from(target_position) else {
            continue;
        };
        let Some(target_type) = program.primitive_type_reference(target_parameter.type_reference)
        else {
            // An erased contract-term formal records a
            // proof term under the same authored
            // argument ordinal the scalar lane uses.
            if target_parameter.relevance.is_erased()
                && proof_only.contract_term_carrier(program, target_parameter.type_reference)
            {
                if let Some(term) = crate::values::scalar::call_lowering::lower_proof_term(
                    program,
                    operators,
                    *argument,
                    scalar_parameters,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    proof_only,
                ) {
                    proof_terms.push(checked_trees::CheckedLocatedProofTerm {
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
                    });
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
        let Some(expression) = lower_return_expression(
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
