//! Building the checked scalar expression plans of one program: the scalar
//! locals, the plan walk and the closed literal guards it lowers.

use crate::values::scalar::boolean_lowering::lower_boolean_guard;
use crate::values::scalar::call_lowering::{
    lower_call_arguments, lower_direct_call_binding_arguments, retain_call_arguments,
    scalar_qualified_call_expression,
};
use crate::values::scalar::expression_facts::operator_is_builtin;
use crate::values::scalar::machine_parameter_booleans::lower_machine_parameter_boolean_expression;
use crate::values::scalar::scalar_lowering::{lower_index_expression, lower_return_expression};
use crate::values::scalar::semantic_casts;
use checked_trees::{
    CheckedBooleanExpression, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
pub(crate) struct ScalarLocal {
    pub(crate) is_mutable: bool,
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) name: String,
    pub(crate) primitive_type: PrimitiveType,
    pub(crate) arithmetic_domain: ArithmeticDomain,
}

pub(crate) fn build_checked_scalar_expression_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CheckedScalarExpressionPlans {
    let mut expressions = Vec::new();
    let mut source_bindings = arena::Arena::default();
    let mut binding_symbols = arena::Arena::default();
    for machine in program.machines() {
        let states = program.machine_states(machine);
        for state in states {
            let mut locals = Vec::new();
            let parameters = program.state_parameters(state);
            let scalar_parameters = parameters
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameter_types = scalar_parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
                .expect("filtered scalar parameters retain primitive carriers");
            let result_type = program.primitive_type_reference(state.return_type);
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                let array_destination = match statement {
                    StatementNode::LocalData(local) if !local.is_mutable => {
                        Some((local.initial_value, local.type_reference, local.symbol))
                    }
                    StatementNode::Expression(expression) => Some((
                        *expression,
                        state.return_type,
                        symbols::SymbolHandle::invalid(),
                    )),
                    _ => None,
                };
                if let Some((expression, expected, destination)) = array_destination
                    && let Some(elements) = validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        expression,
                        expected,
                    )
                {
                    for (element_index, (element, primitive_type)) in
                        elements.elements.into_iter().enumerate()
                    {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        let Some(value) = lower_return_expression(
                            program,
                            operators,
                            element,
                            &scalar_parameters,
                            parameters,
                            &parameter_types,
                            &locals,
                            primitive_type,
                            exact_integer_casts,
                        ) else {
                            continue;
                        };
                        let role = CheckedScalarExpressionRole::ArrayElement {
                            source: checked_trees::CheckedArrayConstructionSource::Statement,
                            element_ordinal,
                        };
                        source_bindings.append(CheckedScalarExpressionBindings {
                            destination,
                            state: state.symbol,
                            statement_ordinal,
                            role,
                            expression: element,
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
                            expression: value,
                        });
                    }
                }
                match statement {
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        // Structural call results establish their own operation
                        // place even when the local later lends mutable access.
                        // Their scalar operands still need exact source rows.
                        if (!local.is_mutable
                            || program
                                .primitive_type_reference(local.type_reference)
                                .is_none())
                            && let ExpressionNode::Call(call) =
                                program.expression_table.expression(local.initial_value)
                            && let Some(arguments) = lower_call_arguments(
                                program,
                                operators,
                                state,
                                statement_ordinal,
                                0,
                                &crate::semantic_calls::CallSite::Expression {
                                    expression: local.initial_value,
                                    call,
                                },
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                        {
                            retain_call_arguments(
                                arguments,
                                &scalar_parameters,
                                &locals,
                                &mut expressions,
                                &mut source_bindings,
                                &mut binding_symbols,
                            );
                        }
                        let Some(primitive_type) =
                            program.primitive_type_reference(local.type_reference)
                        else {
                            continue;
                        };
                        let binding_ordinal = u32::try_from(
                            locals
                                .iter()
                                .filter(|local: &&ScalarLocal| !local.is_mutable)
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
                                    &scalar_parameters,
                                    parameters,
                                    &parameter_types,
                                    &locals,
                                    exact_integer_casts,
                                )
                            {
                                retain_call_arguments(
                                    arguments,
                                    &scalar_parameters,
                                    &locals,
                                    &mut expressions,
                                    &mut source_bindings,
                                    &mut binding_symbols,
                                );
                            } else if let Some(initializer) = lower_return_expression(
                                program,
                                operators,
                                local.initial_value,
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                primitive_type,
                                exact_integer_casts,
                            )
                            .or_else(|| {
                                (primitive_type == PrimitiveType::Bool
                                    && locals.is_empty()
                                    && program
                                        .machine_states(machine)
                                        .first()
                                        .is_some_and(|entry| entry.symbol == state.symbol))
                                .then(|| {
                                    lower_machine_parameter_boolean_expression(
                                        program,
                                        operators,
                                        machine,
                                        local.initial_value,
                                        exact_integer_casts,
                                    )
                                    .map(Box::new)
                                    .map(CheckedScalarExpression::Boolean)
                                })
                                .flatten()
                            }) {
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
                                                    .filter(|local| !local.is_mutable)
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
                            arithmetic_domain: program
                                .arithmetic_domain_for_type_reference(local.type_reference),
                        });
                    }
                    StatementNode::Expression(expression) => {
                        if let ExpressionNode::StructLiteral(literal) =
                            program.expression_table.expression(*expression)
                            && let Some(case_symbol) = literal.case_symbol
                            && let Some(data) = program
                                .data_definitions()
                                .iter()
                                .find(|data| data.symbol == literal.type_symbol)
                            && let Some(typed_trees::data::DataMember::Variant(variant)) =
                                program.data_members(data).iter().find(|member| {
                                    matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == case_symbol)
                                })
                        {
                            for (field_index, field) in program
                                .expression_table
                                .struct_fields(literal.fields)
                                .iter()
                                .enumerate()
                            {
                                let Some(primitive_type) = program
                                    .data_payload_fields(variant)
                                    .iter()
                                    .find(|declaration| declaration.symbol == field.field_symbol)
                                    .and_then(|declaration| program.primitive_type_reference(declaration.type_reference))
                                else {
                                    continue;
                                };
                                let Ok(field_ordinal) = u32::try_from(field_index) else {
                                    continue;
                                };
                                let Some(value) = lower_return_expression(
                                    program,
                                    operators,
                                    field.value,
                                    &scalar_parameters,
                                    parameters,
                                    &parameter_types,
                                    &locals,
                                    primitive_type,
                                    exact_integer_casts,
                                ) else {
                                    continue;
                                };
                                let role = CheckedScalarExpressionRole::ReturnCaseField { field_ordinal };
                                source_bindings.append(CheckedScalarExpressionBindings {
                                    destination: symbols::SymbolHandle::invalid(),
                                    state: state.symbol,
                                    statement_ordinal,
                                    role,
                                    expression: field.value,
                                    symbols: binding_symbols.insert_many(
                                        scalar_parameters.iter().map(|parameter| parameter.symbol)
                                            .chain(locals.iter().filter(|local| !local.is_mutable).map(|local| local.symbol)),
                                    ),
                                });
                                expressions.push(CheckedLocatedScalarExpression {
                                    state: state.symbol,
                                    statement_ordinal,
                                    role,
                                    expression: value,
                                });
                            }
                        }
                        let unit_statement = validation::unit_statement_call_is_supported(
                            program,
                            machine,
                            state,
                            *expression,
                        );
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*expression)
                            && let Some(arguments) = lower_call_arguments(
                                program,
                                operators,
                                state,
                                statement_ordinal,
                                0,
                                &crate::semantic_calls::CallSite::Expression {
                                    expression: *expression,
                                    call,
                                },
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                        {
                            retain_call_arguments(
                                arguments,
                                &scalar_parameters,
                                &locals,
                                &mut expressions,
                                &mut source_bindings,
                                &mut binding_symbols,
                            );
                        }
                        if !unit_statement
                            && let Some(result_type) = result_type
                            && let Some(return_expression) = lower_return_expression(
                                program,
                                operators,
                                *expression,
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                result_type,
                                exact_integer_casts,
                            )
                        {
                            source_bindings.append(CheckedScalarExpressionBindings {
                                destination: symbols::SymbolHandle::invalid(),
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Return,
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
                                role: CheckedScalarExpressionRole::Return,
                                expression: return_expression,
                            });
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        if let ExpressionNode::Indexed(indexed) =
                            program.expression_table.expression(assignment.target)
                            && !matches!(
                                program.expression_table.expression(indexed.index),
                                ExpressionNode::Range(_)
                            )
                            && let Some(expression) = lower_index_expression(
                                program,
                                operators,
                                indexed.index,
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                        {
                            source_bindings.append(CheckedScalarExpressionBindings {
                                destination: symbols::SymbolHandle::invalid(),
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::AssignmentIndex,
                                expression: indexed.index,
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
                                role: CheckedScalarExpressionRole::AssignmentIndex,
                                expression,
                            });
                        }
                        // A call delivering its result to an assignment target
                        // needs the same scalar-argument custody rows as one
                        // delivering it to a local: the arguments are evaluated
                        // and transferred identically, and where the result
                        // lands does not change their source bindings. The
                        // qualified-call spelling below stays for the cast
                        // chains only ordinary integer-returning machines have.
                        if let Some(expression) =
                            scalar_qualified_call_expression(program, assignment.value).or_else(
                                || {
                                    matches!(
                                        program.expression_table.expression(assignment.value),
                                        ExpressionNode::Call(_)
                                    )
                                    .then_some(assignment.value)
                                },
                            )
                            && let ExpressionNode::Call(call) =
                                program.expression_table.expression(expression)
                            && let Some(arguments) = lower_call_arguments(
                                program,
                                operators,
                                state,
                                statement_ordinal,
                                0,
                                &crate::semantic_calls::CallSite::Expression { expression, call },
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                        {
                            retain_call_arguments(
                                arguments,
                                &scalar_parameters,
                                &locals,
                                &mut expressions,
                                &mut source_bindings,
                                &mut binding_symbols,
                            );
                        }
                        // Retain selected RHS meaning at the statement. A later
                        // executable consumer still owns its admitted store shape.
                        let Some(target_type_reference) =
                            crate::flow::expression_type_reference_in_state(
                                program,
                                state.symbol,
                                statement_index,
                                assignment.target,
                            )
                        else {
                            continue;
                        };
                        let Some(target_type) =
                            assignment_target_primitive_type(program, target_type_reference)
                        else {
                            continue;
                        };
                        let Some(expression) = lower_return_expression(
                            program,
                            operators,
                            assignment.value,
                            &scalar_parameters,
                            parameters,
                            &parameter_types,
                            &locals,
                            target_type,
                            exact_integer_casts,
                        ) else {
                            continue;
                        };
                        source_bindings.append(CheckedScalarExpressionBindings {
                            destination: match program
                                .expression_table
                                .expression(assignment.target)
                            {
                                ExpressionNode::Name(path) => path.symbol,
                                _ => symbols::SymbolHandle::invalid(),
                            },
                            state: state.symbol,
                            statement_ordinal,
                            role: CheckedScalarExpressionRole::AssignmentValue,
                            expression: assignment.value,
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
                            role: CheckedScalarExpressionRole::AssignmentValue,
                            expression,
                        });
                    }
                    StatementNode::Call(call) => {
                        if let Some(arguments) = lower_call_arguments(
                            program,
                            operators,
                            state,
                            statement_ordinal,
                            0,
                            &crate::semantic_calls::CallSite::Statement(call),
                            &scalar_parameters,
                            parameters,
                            &parameter_types,
                            &locals,
                            exact_integer_casts,
                        ) {
                            retain_call_arguments(
                                arguments,
                                &scalar_parameters,
                                &locals,
                                &mut expressions,
                                &mut source_bindings,
                                &mut binding_symbols,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(authored_guard) = transition.guard
                            && let Some(guard) = lower_boolean_guard(
                                program,
                                operators,
                                authored_guard,
                                &scalar_parameters,
                                parameters,
                                &parameter_types,
                                &locals,
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
                        for (target, continuation) in
                            [(transition.target, false), (transition.continuation, true)]
                        {
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
                                    &scalar_parameters,
                                    parameters,
                                    &parameter_types,
                                    &locals,
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
                            let Some(target_state) =
                                crate::checks::termination::named_transition_target_state_index(
                                    program,
                                    machine,
                                    path.symbol,
                                )
                                .and_then(|target_index| states.get(target_index))
                            else {
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
                                let Some(target_type) = program
                                    .primitive_type_reference(target_parameter.type_reference)
                                else {
                                    if continuation {
                                        continue;
                                    }
                                    let ExpressionNode::Indexed(indexed) =
                                        program.expression_table.expression(*argument)
                                    else {
                                        continue;
                                    };
                                    let ExpressionNode::Range(range) =
                                        program.expression_table.expression(indexed.index)
                                    else {
                                        continue;
                                    };
                                    if range.end_inclusive
                                        || operators.expression_use(*argument).is_some_and(|selected| {
                                            selected.spelling != language_core::OperatorSpelling::Range
                                                || selected.selected_operator_symbol.is_valid()
                                                || selected.candidate_count != 0
                                                || !matches!(selected.status,
                                                    CheckedOperatorResolutionStatus::Missing
                                                        | CheckedOperatorResolutionStatus::BuiltinFallback)
                                        })
                                    {
                                        continue;
                                    }
                                    for (endpoint, role) in [
                                        (
                                            range.start,
                                            CheckedScalarExpressionRole::TransitionSubsliceStart {
                                                argument_ordinal,
                                            },
                                        ),
                                        (
                                            range.end,
                                            CheckedScalarExpressionRole::TransitionSubsliceEnd {
                                                argument_ordinal,
                                            },
                                        ),
                                    ] {
                                        if !endpoint.is_valid() {
                                            continue;
                                        }
                                        let Some(expression) = lower_return_expression(
                                            program,
                                            operators,
                                            endpoint,
                                            &scalar_parameters,
                                            parameters,
                                            &parameter_types,
                                            &locals,
                                            PrimitiveType::U64,
                                            exact_integer_casts,
                                        ) else {
                                            continue;
                                        };
                                        source_bindings.append(CheckedScalarExpressionBindings {
                                            destination: symbols::SymbolHandle::invalid(),
                                            state: state.symbol,
                                            statement_ordinal,
                                            role,
                                            expression: endpoint,
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
                                    continue;
                                };
                                let Some(expression) = lower_return_expression(
                                    program,
                                    operators,
                                    *argument,
                                    &scalar_parameters,
                                    parameters,
                                    &parameter_types,
                                    &locals,
                                    target_type,
                                    exact_integer_casts,
                                ) else {
                                    continue;
                                };
                                let role = if continuation {
                                    CheckedScalarExpressionRole::TransitionContinuationArgument {
                                        argument_ordinal,
                                    }
                                } else {
                                    CheckedScalarExpressionRole::TransitionArgument {
                                        argument_ordinal,
                                    }
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
                    _ => {}
                }
            }
        }
    }
    // A pure payload tree cannot carry an authored semantic qualification
    // transfer. Leave those exact source bindings to the computation graph.
    let mut retained_bindings = arena::Arena::default();
    for (_, binding) in source_bindings.iter() {
        if semantic_casts::requires_custody(program, binding.state, binding.expression) {
            expressions.retain(|expression| {
                expression.state != binding.state
                    || expression.statement_ordinal != binding.statement_ordinal
                    || expression.role != binding.role
            });
        } else {
            retained_bindings.append(binding.clone());
        }
    }
    CheckedScalarExpressionPlans {
        expressions,
        source_bindings: retained_bindings,
        binding_symbols,
    }
}

pub(crate) fn lower_closed_integer_literal_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    mut expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    // Successful checking has already established literal landing and selected
    // comparison meaning. Comparing these immutable payloads needs no runtime
    // arithmetic, whether the literals are anonymous or have declared carriers.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == BinaryOperator::Equal
        && operator_is_builtin(operators, expression)
    {
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(true), _) => expression = binary.right,
            (_, ExpressionNode::Boolean(true)) => expression = binary.left,
            _ => {}
        }
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if !operator_is_builtin(operators, expression) {
        return None;
    }
    let ExpressionNode::Integer(left) = program.expression_table.expression(binary.left) else {
        return None;
    };
    let ExpressionNode::Integer(right) = program.expression_table.expression(binary.right) else {
        return None;
    };
    let left = left.value_bignum()?;
    let right = right.value_bignum()?;
    let value = match binary.operator {
        BinaryOperator::Equal => left == right,
        BinaryOperator::NotEqual => left != right,
        BinaryOperator::Less => left < right,
        BinaryOperator::LessOrEqual => left <= right,
        BinaryOperator::Greater => left > right,
        BinaryOperator::GreaterOrEqual => left >= right,
        _ => return None,
    };
    Some(CheckedBooleanExpression::Constant(value))
}

pub(crate) fn assignment_target_primitive_type(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut crossed_reference = false;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } if !crossed_reference => {
                crossed_reference = true;
                type_reference = *referee;
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => return program.primitive_type_reference(type_reference),
        }
    }
}
