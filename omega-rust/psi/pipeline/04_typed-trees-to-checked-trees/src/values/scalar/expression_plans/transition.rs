//! A transition: its guard (`Guard`), a returned arm value (`Return` or
//! `ContinuationReturn`), and each successor argument as a scalar
//! expression, a subslice endpoint or an erased proof term.

use super::{
    CheckedLocatedScalarExpression, CheckedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionRole, PrimitiveType, StatementPlanner, TransitionGuardNode,
    TransitionTargetNode, lower_boolean_guard, lower_return_expression, retain_subslice_endpoints,
};

pub(super) fn plan(
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
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::Guard,
            expression: authored_guard,
            symbols: binding_symbols.insert_many(
                scalar_parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        locals
                            .iter()
                            .filter(|local| !local.is_mutable)
                            .map(|local| local.symbol),
                    ),
            ),
        });
        expressions.push(CheckedLocatedScalarExpression {
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::Guard,
            expression: CheckedScalarExpression::Boolean(Box::new(guard)),
        });
    }
    for (target, continuation) in [(transition.target, false), (transition.continuation, true)] {
        if !target.is_valid() {
            continue;
        }
        if transition.exit == typed_trees::statement::TransitionExit::Ordinary
            && let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
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
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: symbols::SymbolHandle::invalid(),
                state: state.symbol,
                statement_ordinal,
                role,
                expression: *expression,
                symbols: binding_symbols.insert_many(
                    scalar_parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .chain(
                            locals
                                .iter()
                                .filter(|local| !local.is_mutable)
                                .map(|local| local.symbol),
                        ),
                ),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role,
                expression: return_expression,
            });
        }
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(target)
        else {
            continue;
        };
        let Some(target_state) = crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        )
        .and_then(|target_index| states.get(target_index))
        .or_else(|| {
            // A target spelling another machine's entry
            // names that machine's first state; its
            // formals pair with the same authored
            // ordinals the in-machine walk uses.
            crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)
                .map(|(_, entry)| entry)
        }) else {
            continue;
        };
        let target_parameters = program
            .state_parameters(target_state)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| !parameter.is_self);
        for (argument, (target_position, target_parameter)) in program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .zip(target_parameters)
        {
            let Ok(argument_ordinal) = u32::try_from(target_position) else {
                continue;
            };
            let Some(target_type) =
                program.primitive_type_reference(target_parameter.type_reference)
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
                let endpoints = super::super::subslice_endpoints::subslice_endpoints(
                    program,
                    operators,
                    *argument,
                    checked_trees::CheckedSubsliceSite::TransitionArgument { argument_ordinal },
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
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: target_parameter.symbol,
                state: state.symbol,
                statement_ordinal,
                role,
                expression: *argument,
                symbols: binding_symbols.insert_many(
                    scalar_parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .chain(
                            locals
                                .iter()
                                .filter(|local| !local.is_mutable)
                                .map(|local| local.symbol),
                        ),
                ),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role,
                expression,
            });
        }
    }
}
