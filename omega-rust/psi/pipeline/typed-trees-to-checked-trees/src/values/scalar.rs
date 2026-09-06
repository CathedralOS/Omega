use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedIntegerRange, CheckedLocatedScalarExpression,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
    CheckedScalarExpressionBindings, CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
    CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
};
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, LandedIntegerType},
};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    signature::StateParameter,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};

mod call_arguments;
mod computations;
mod contract_entry;
mod result_contract;
pub(crate) use call_arguments::{
    nested_structural_call_return_type, retain_nested_structural_call_arguments,
};
pub(crate) use computations::build_checked_scalar_computation_plans;
pub(crate) use contract_entry::{
    lower_machine_entry_boolean_expression, lower_machine_entry_scalar_contract_expression,
};
pub(crate) use result_contract::{
    lower_integer_contract_predicate, lower_integer_parameter_range_requirements,
};

#[derive(Debug, Clone)]
struct ScalarLocal {
    is_mutable: bool,
    symbol: symbols::SymbolHandle,
    name: String,
    primitive_type: PrimitiveType,
    arithmetic_domain: ArithmeticDomain,
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
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
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
                match statement {
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        if !local.is_mutable
                            && let ExpressionNode::Call(call) =
                                program.expression_table.expression(local.initial_value)
                            && let Some(arguments) = lower_boundary_call_arguments(
                                program,
                                operators,
                                state,
                                statement_ordinal,
                                0,
                                &crate::CallSite::Expression {
                                    expression: local.initial_value,
                                    call,
                                },
                                &scalar_parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                                true,
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
                        let unit_statement = validation::unit_statement_call_is_supported(
                            program,
                            machine,
                            state,
                            *expression,
                        );
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*expression)
                            && let Some(arguments) = lower_boundary_call_arguments(
                                program,
                                operators,
                                state,
                                statement_ordinal,
                                0,
                                &crate::CallSite::Expression {
                                    expression: *expression,
                                    call,
                                },
                                &scalar_parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                                unit_statement,
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
                        if let Some(arguments) = lower_boundary_call_arguments(
                            program,
                            operators,
                            state,
                            statement_ordinal,
                            0,
                            &crate::CallSite::Statement(call),
                            &scalar_parameters,
                            &parameter_types,
                            &locals,
                            exact_integer_casts,
                            true,
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
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                            .or_else(|| {
                                lower_closed_integer_literal_guard(
                                    program,
                                    operators,
                                    authored_guard,
                                )
                            })
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
                            let Some(target_state) = states
                                .iter()
                                .find(|candidate| candidate.symbol == path.symbol)
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
                                let Some(target_type) = program
                                    .primitive_type_reference(target_parameter.type_reference)
                                else {
                                    continue;
                                };
                                let Some(expression) = lower_return_expression(
                                    program,
                                    operators,
                                    *argument,
                                    &scalar_parameters,
                                    &parameter_types,
                                    &locals,
                                    target_type,
                                    exact_integer_casts,
                                ) else {
                                    continue;
                                };
                                let Ok(argument_ordinal) = u32::try_from(target_position) else {
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
    CheckedScalarExpressionPlans {
        expressions,
        source_bindings,
        binding_symbols,
    }
}

fn lower_closed_integer_literal_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    mut expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    // Const substitution can leave a comparison of two anonymous literals:
    // there is deliberately no runtime carrier to land. Preserve its exact
    // mathematical result as a checked Boolean constant instead of guessing
    // an integer width downstream.
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

fn assignment_target_primitive_type(
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

fn retain_call_arguments(
    arguments: Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>,
    parameters: &[StateParameter],
    locals: &[ScalarLocal],
    expressions: &mut Vec<CheckedLocatedScalarExpression>,
    source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
    binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
) {
    for (authored_argument, argument) in arguments {
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: argument.state,
            statement_ordinal: argument.statement_ordinal,
            role: argument.role,
            expression: authored_argument,
            symbols: binding_symbols.insert_many(
                parameters.iter().map(|parameter| parameter.symbol).chain(
                    locals
                        .iter()
                        .filter(|local| !local.is_mutable)
                        .map(|local| local.symbol),
                ),
            ),
        });
        expressions.push(argument);
    }
}

fn call_is_boundary(program: &TypedTrees, target_symbol: symbols::SymbolHandle) -> bool {
    let requirement_symbol = program
        .machine_parameter_signature(target_symbol)
        .map_or(target_symbol, |(_, signature)| signature.symbol);
    program.machines().iter().any(|machine| {
        machine.supply_mode.is_boundary_declaration()
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_symbol)
    }) || program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == requirement_symbol)
    }) || validation::exact_compiler_intrinsic_boundary_requirement(program, target_symbol)
        .is_some()
}

#[allow(clippy::too_many_arguments)]
fn lower_boundary_call_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    call_ordinal: usize,
    call_site: &crate::CallSite<'_>,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    admit_internal_unit: bool,
) -> Option<Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>> {
    let target_symbol = match call_site {
        crate::CallSite::Statement(call) => call.target_symbol,
        crate::CallSite::Expression { call, .. } => call.target_symbol,
        crate::CallSite::TransitionNamed { .. } => return None,
    };
    let is_boundary = call_is_boundary(program, target_symbol);
    if !is_boundary && !admit_internal_unit {
        return None;
    }

    let target_parameters = crate::call_target_parameters(program, target_symbol)?;
    let explicit_arguments = crate::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut scalar_index = 0usize;
    let mut output = Vec::new();
    for target in target_parameters {
        if target.is_self && !explicit_self {
            continue;
        }
        let argument = *explicit_arguments.get(explicit_index)?;
        explicit_index = explicit_index.checked_add(1)?;
        let Some(expected_type) = program.primitive_type_reference(target.type_reference) else {
            continue;
        };
        if target.is_self
            || target.is_const
            || (target.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, target).is_none())
        {
            return None;
        }
        let lowered = lower_return_expression(
            program,
            operators,
            argument,
            parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        );
        if let Some(lowered) = lowered {
            output.push((
                argument,
                CheckedLocatedScalarExpression {
                    state: state.symbol,
                    statement_ordinal,
                    role: if is_boundary {
                        CheckedScalarExpressionRole::BoundaryCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    } else {
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    },
                    expression: lowered,
                },
            ));
        }
        scalar_index = scalar_index.checked_add(1)?;
    }
    (explicit_index == explicit_arguments.len()).then_some(output)
}

#[allow(clippy::too_many_arguments)]
fn lower_direct_call_binding_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == call.target_symbol)
    })?;
    let target_parameters = crate::call_target_parameters(program, call.target_symbol)?;
    if target_parameters.iter().any(|parameter| {
        parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
    }) {
        return None;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    arguments
        .iter()
        .zip(target_parameters)
        .enumerate()
        .map(|(argument_index, (argument, target_parameter))| {
            let expected_type =
                program.primitive_type_reference(target_parameter.type_reference)?;
            Some((
                *argument,
                CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal,
                    role: CheckedScalarExpressionRole::CallArgument {
                        binding_ordinal,
                        argument_ordinal: u32::try_from(argument_index).ok()?,
                    },
                    expression: lower_return_expression(
                        program,
                        operators,
                        *argument,
                        parameters,
                        parameter_types,
                        locals,
                        expected_type,
                        exact_integer_casts,
                    )?,
                },
            ))
        })
        .collect()
}

/// Lower a contract predicate in the selected machine's entry-parameter
/// namespace. Crash contracts use this to retain the same checked scalar
/// meaning as executable guards without carrying typed-tree handles into the
/// terminal producer.
pub(crate) fn lower_machine_parameter_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    fn structural_parameter_field_path(
        program: &TypedTrees,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        fields: &mut Vec<CheckedStructuralPredicatePathSegment>,
    ) -> Option<u32> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                    program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .iter()
                        .copied()
                        .find(|symbol| symbol.is_valid())
                });
                let name_text = program
                    .expression_table
                    .name_path_members(name.members)
                    .last();
                parameters
                    .iter()
                    .position(|parameter| {
                        name_symbol.is_some_and(|symbol| parameter.symbol == symbol)
                            || name_text.is_some_and(|name| parameter.name == *name)
                    })
                    .and_then(|position| u32::try_from(position).ok())
            }
            ExpressionNode::Member(member) => {
                let parameter =
                    structural_parameter_field_path(program, parameters, member.receiver, fields)?;
                let field_identity = |field: &typed_trees::data::DataField| {
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned())
                };
                if let Some(case_name) = &member.case_variant {
                    let (case, field) = program.data_definitions().iter().find_map(|data| {
                        program.data_members(data).iter().find_map(|candidate| {
                            let typed_trees::data::DataMember::Variant(variant) = candidate else {
                                return None;
                            };
                            if variant.name != *case_name {
                                return None;
                            }
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == member.member_symbol)
                                .map(|field| {
                                    (
                                        variant
                                            .identity
                                            .map(|identity| format!("#{identity}"))
                                            .unwrap_or_else(|| variant.name.as_str().to_owned()),
                                        field_identity(field),
                                    )
                                })
                        })
                    })?;
                    fields.push(CheckedStructuralPredicatePathSegment::Case(case));
                    fields.push(CheckedStructuralPredicatePathSegment::Field(field));
                } else {
                    let identity = if member.member_symbol.is_valid() {
                        program.data_definitions().iter().find_map(|data| {
                            program.data_members(data).iter().find_map(|candidate| {
                                let typed_trees::data::DataMember::Field(field) = candidate else {
                                    return None;
                                };
                                (field.symbol == member.member_symbol)
                                    .then(|| field_identity(field))
                            })
                        })?
                    } else {
                        // Contract member expressions can reach this carrier
                        // before their field symbol is retained. Keep the
                        // authored segment in that case: path_type_reference
                        // resolves it against the exact receiver type below,
                        // so this does not perform global name-based selection.
                        member.member.as_str().to_owned()
                    };
                    fields.push(CheckedStructuralPredicatePathSegment::Field(identity));
                }
                Some(parameter)
            }
            _ => None,
        }
    }

    fn lower_structural_boolean_expression(
        program: &TypedTrees,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        fn field_type(
            program: &TypedTrees,
            mut receiver: TypeReferenceHandle,
            identity: &str,
        ) -> Option<TypeReferenceHandle> {
            loop {
                match program.type_reference_table.type_reference(receiver) {
                    TypeReferenceNode::Reference { referee, .. }
                    | TypeReferenceNode::Constrained {
                        base_type: referee, ..
                    } => receiver = *referee,
                    TypeReferenceNode::Named { symbol, name }
                    | TypeReferenceNode::Generic {
                        base_symbol: symbol,
                        base_name: name,
                        ..
                    } => {
                        let data = program.data_definitions().iter().find(|data| {
                            (symbol.is_valid() && data.symbol == *symbol) || data.name == *name
                        })?;
                        return program.data_members(data).iter().find_map(|member| {
                            let typed_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };
                            (field.name.as_str() == identity).then_some(field.type_reference)
                        });
                    }
                    _ => return None,
                }
            }
        }

        fn path_type_reference(
            program: &TypedTrees,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<TypeReferenceHandle> {
            let parameter = usize::try_from(parameter_position)
                .ok()
                .and_then(|position| parameters.get(position))?;
            let mut receiver = parameter.type_reference;
            let mut selected_case = None;
            for segment in path {
                match segment {
                    CheckedStructuralPredicatePathSegment::Case(case) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        let data = structural_data(program, receiver)?;
                        let variant = program.data_members(data).iter().find_map(|member| {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                return None;
                            };
                            let identity = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            (identity == *case).then_some(variant)
                        })?;
                        selected_case = Some(variant);
                    }
                    CheckedStructuralPredicatePathSegment::Field(field) => {
                        receiver = if let Some(variant) = selected_case.take() {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find_map(|candidate| {
                                    let identity = candidate
                                        .identity
                                        .map(|identity| format!("#{identity}"))
                                        .unwrap_or_else(|| candidate.name.as_str().to_owned());
                                    (identity == *field).then_some(candidate.type_reference)
                                })?
                        } else {
                            field_type(program, receiver, field)?
                        };
                    }
                }
            }
            selected_case.is_none().then_some(receiver)
        }

        fn path_primitive_type(
            program: &TypedTrees,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<PrimitiveType> {
            program.primitive_type_reference(path_type_reference(
                program,
                parameters,
                parameter_position,
                path,
            )?)
        }

        fn structural_data(
            program: &TypedTrees,
            mut type_reference: TypeReferenceHandle,
        ) -> Option<&typed_trees::data::DataDefinition> {
            let (symbol, name) = loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Reference { referee, .. }
                    | TypeReferenceNode::Constrained {
                        base_type: referee, ..
                    } => type_reference = *referee,
                    TypeReferenceNode::Named { symbol, name }
                    | TypeReferenceNode::Generic {
                        base_symbol: symbol,
                        base_name: name,
                        ..
                    } => break (*symbol, name),
                    _ => return None,
                }
            };
            program
                .data_definitions()
                .iter()
                .find(|data| (symbol.is_valid() && data.symbol == symbol) || data.name == *name)
        }

        fn structural_record_fields(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Vec<&typed_trees::data::DataField>> {
            let data = structural_data(program, type_reference)?;
            let mut fields = Vec::new();
            for member in program.data_members(data) {
                let typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                if field.relevance.is_erased() {
                    return None;
                }
                fields.push(field);
            }
            Some(fields)
        }

        fn payloadless_sum_cases(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Vec<String>> {
            let data = structural_data(program, type_reference)?;
            let members = program.data_members(data);
            if !matches!(
                typed_trees::data::DataDefinition::shape_kind_from_members(members),
                typed_trees::data::DataShapeKind::Enum
            ) {
                return None;
            }
            members
                .iter()
                .map(|member| {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    program.data_payload_fields(variant).is_empty().then(|| {
                        variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned())
                    })
                })
                .collect()
        }

        fn append_direct_structural_leaf_equality(
            program: &TypedTrees,
            left_parameter: u32,
            right_parameter: u32,
            type_reference: TypeReferenceHandle,
            left: Vec<CheckedStructuralPredicatePathSegment>,
            right: Vec<CheckedStructuralPredicatePathSegment>,
            comparisons: &mut Vec<CheckedBooleanExpression>,
        ) -> Option<()> {
            match program.primitive_type_reference(type_reference) {
                Some(PrimitiveType::Bool) => {
                    comparisons.push(CheckedBooleanExpression::Equal {
                        left: Box::new(CheckedBooleanExpression::StructuralParameterField {
                            parameter_position: left_parameter,
                            path: left,
                        }),
                        right: Box::new(CheckedBooleanExpression::StructuralParameterField {
                            parameter_position: right_parameter,
                            path: right,
                        }),
                    });
                }
                Some(primitive_type)
                    if is_integer(primitive_type) && primitive_type != PrimitiveType::Addr =>
                {
                    comparisons.push(CheckedBooleanExpression::IntegerComparison {
                        kind: CheckedIntegerComparisonKind::Equal,
                        left: Box::new(CheckedScalarExpression::StructuralParameterField {
                            parameter_position: left_parameter,
                            path: left,
                            primitive_type,
                        }),
                        right: Box::new(CheckedScalarExpression::StructuralParameterField {
                            parameter_position: right_parameter,
                            path: right,
                            primitive_type,
                        }),
                    });
                }
                Some(primitive_type)
                    if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) =>
                {
                    let mut left = CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left,
                    };
                    let mut right = CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right,
                    };
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    comparisons.push(CheckedBooleanExpression::IeeeFloatComparison {
                        kind: CheckedIeeeFloatComparisonKind::Equal,
                        primitive_type,
                        left,
                        right,
                    });
                }
                Some(_) => return None,
                None if crate::flow::byte_sequence_carrier(program, type_reference, &[])
                    .is_some() =>
                {
                    let mut left = CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left,
                    };
                    let mut right = CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right,
                    };
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    comparisons.push(CheckedBooleanExpression::ByteSequenceEqual { left, right });
                }
                None => return None,
            }
            Some(())
        }

        fn is_bounded_nested_mixed_field_path_pair(
            left: &[CheckedStructuralPredicatePathSegment],
            right: &[CheckedStructuralPredicatePathSegment],
        ) -> bool {
            const MAX_ENCLOSING_FIELDS: usize = 14;

            !left.is_empty()
                && left.len() == right.len()
                && left.len() <= MAX_ENCLOSING_FIELDS
                && left.iter().all(|segment| {
                    matches!(segment, CheckedStructuralPredicatePathSegment::Field(_))
                })
                && right.iter().all(|segment| {
                    matches!(segment, CheckedStructuralPredicatePathSegment::Field(_))
                })
        }

        fn append_acyclic_structural_equality(
            program: &TypedTrees,
            left_parameter: u32,
            right_parameter: u32,
            type_reference: TypeReferenceHandle,
            left_path: &[CheckedStructuralPredicatePathSegment],
            right_path: &[CheckedStructuralPredicatePathSegment],
            comparisons: &mut Vec<CheckedBooleanExpression>,
            visiting: &mut Vec<symbols::SymbolHandle>,
            nested_mixed_seen: &mut bool,
            allow_direct_nested_mixed: bool,
        ) -> Option<()> {
            if append_direct_structural_leaf_equality(
                program,
                left_parameter,
                right_parameter,
                type_reference,
                left_path.to_vec(),
                right_path.to_vec(),
                comparisons,
            )
            .is_some()
            {
                return Some(());
            }
            if let Some(cases) = payloadless_sum_cases(program, type_reference) {
                comparisons.push(CheckedBooleanExpression::PayloadlessSumEqual {
                    left: CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left_path.to_vec(),
                    },
                    right: CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right_path.to_vec(),
                    },
                    cases,
                });
                return Some(());
            }
            let data = structural_data(program, type_reference)?;
            if !data.symbol.is_valid() || visiting.contains(&data.symbol) {
                return None;
            }
            visiting.push(data.symbol);
            let result = (|| {
                let members = program.data_members(data);
                match typed_trees::data::DataDefinition::shape_kind_from_members(members) {
                    typed_trees::data::DataShapeKind::Empty => Some(()),
                    typed_trees::data::DataShapeKind::Record => {
                        for field in structural_record_fields(program, type_reference)? {
                            let field_identity = field
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| field.name.as_str().to_owned());
                            let mut left = left_path.to_vec();
                            left.push(CheckedStructuralPredicatePathSegment::Field(
                                field_identity.clone(),
                            ));
                            let mut right = right_path.to_vec();
                            right
                                .push(CheckedStructuralPredicatePathSegment::Field(field_identity));
                            append_acyclic_structural_equality(
                                program,
                                left_parameter,
                                right_parameter,
                                field.type_reference,
                                &left,
                                &right,
                                comparisons,
                                visiting,
                                nested_mixed_seen,
                                allow_direct_nested_mixed,
                            )?;
                        }
                        Some(())
                    }
                    typed_trees::data::DataShapeKind::Enum => {
                        let mut arms = Vec::new();
                        for member in members {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                return None;
                            };
                            let case = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            let mut arm = vec![
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: left_parameter,
                                        path: left_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: right_parameter,
                                        path: right_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                            ];
                            for field in program.data_payload_fields(variant) {
                                if field.relevance.is_erased() {
                                    return None;
                                }
                                let field_identity = field
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| field.name.as_str().to_owned());
                                let mut left = left_path.to_vec();
                                left.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                left.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity.clone(),
                                ));
                                let mut right = right_path.to_vec();
                                right.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                right.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity,
                                ));
                                append_acyclic_structural_equality(
                                    program,
                                    left_parameter,
                                    right_parameter,
                                    field.type_reference,
                                    &left,
                                    &right,
                                    &mut arm,
                                    visiting,
                                    nested_mixed_seen,
                                    allow_direct_nested_mixed,
                                )?;
                            }
                            let mut arm = arm.into_iter();
                            let first = arm.next()?;
                            arms.push(arm.fold(first, |left, right| {
                                CheckedBooleanExpression::And {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                }
                            }));
                        }
                        let mut arms = arms.into_iter();
                        let first = arms.next()?;
                        comparisons.push(arms.fold(first, |left, right| {
                            CheckedBooleanExpression::Or {
                                left: Box::new(left),
                                right: Box::new(right),
                            }
                        }));
                        Some(())
                    }
                    typed_trees::data::DataShapeKind::Mixed => {
                        // The bounded nested mixed-shape slice permits one through
                        // fourteen direct record fields before the sole mixed occurrence.
                        // Deeper records, case payloads, and two mixed siblings retain
                        // their fail-closed fence until their independent path and replay
                        // canaries land.
                        if !left_path.is_empty() || !right_path.is_empty() {
                            if !is_bounded_nested_mixed_field_path_pair(left_path, right_path)
                                || !allow_direct_nested_mixed
                                || *nested_mixed_seen
                            {
                                return None;
                            }
                            *nested_mixed_seen = true;
                        }
                        for member in members {
                            let typed_trees::data::DataMember::Field(field) = member else {
                                continue;
                            };
                            if field.relevance.is_erased() {
                                return None;
                            }
                            let field_identity = field
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| field.name.as_str().to_owned());
                            let mut left = left_path.to_vec();
                            left.push(CheckedStructuralPredicatePathSegment::Field(
                                field_identity.clone(),
                            ));
                            let mut right = right_path.to_vec();
                            right
                                .push(CheckedStructuralPredicatePathSegment::Field(field_identity));
                            append_acyclic_structural_equality(
                                program,
                                left_parameter,
                                right_parameter,
                                field.type_reference,
                                &left,
                                &right,
                                comparisons,
                                visiting,
                                nested_mixed_seen,
                                allow_direct_nested_mixed,
                            )?;
                        }
                        let mut arms = Vec::new();
                        for member in members {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                continue;
                            };
                            let case = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            let mut arm = vec![
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: left_parameter,
                                        path: left_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: right_parameter,
                                        path: right_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                            ];
                            for field in program.data_payload_fields(variant) {
                                if field.relevance.is_erased() {
                                    return None;
                                }
                                let field_identity = field
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| field.name.as_str().to_owned());
                                let mut left = left_path.to_vec();
                                left.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                left.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity.clone(),
                                ));
                                let mut right = right_path.to_vec();
                                right.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                right.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity,
                                ));
                                append_acyclic_structural_equality(
                                    program,
                                    left_parameter,
                                    right_parameter,
                                    field.type_reference,
                                    &left,
                                    &right,
                                    &mut arm,
                                    visiting,
                                    nested_mixed_seen,
                                    allow_direct_nested_mixed,
                                )?;
                            }
                            let mut arm = arm.into_iter();
                            let first = arm.next()?;
                            arms.push(arm.fold(first, |left, right| {
                                CheckedBooleanExpression::And {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                }
                            }));
                        }
                        let mut arms = arms.into_iter();
                        let first = arms.next()?;
                        comparisons.push(arms.fold(first, |left, right| {
                            CheckedBooleanExpression::Or {
                                left: Box::new(left),
                                right: Box::new(right),
                            }
                        }));
                        Some(())
                    }
                }
            })();
            visiting.pop();
            result
        }

        fn lower_structural_equality(
            program: &TypedTrees,
            parameters: &[StateParameter],
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            fn collect_comparisons(
                program: &TypedTrees,
                left_parameter: u32,
                right_parameter: u32,
                type_reference: TypeReferenceHandle,
                left_path: &mut [CheckedStructuralPredicatePathSegment],
                right_path: &mut [CheckedStructuralPredicatePathSegment],
                output: &mut Vec<CheckedBooleanExpression>,
                visiting: &mut Vec<symbols::SymbolHandle>,
                allow_direct_nested_mixed: bool,
            ) -> Option<()> {
                let mut nested_mixed_seen = false;
                append_acyclic_structural_equality(
                    program,
                    left_parameter,
                    right_parameter,
                    type_reference,
                    left_path,
                    right_path,
                    output,
                    visiting,
                    &mut nested_mixed_seen,
                    allow_direct_nested_mixed,
                )
            }

            let mut left_path = Vec::new();
            let mut right_path = Vec::new();
            let left_parameter =
                structural_parameter_field_path(program, parameters, left, &mut left_path)?;
            let right_parameter =
                structural_parameter_field_path(program, parameters, right, &mut right_path)?;
            let left_type = path_type_reference(program, parameters, left_parameter, &left_path)?;
            let right_type =
                path_type_reference(program, parameters, right_parameter, &right_path)?;
            let left_data = structural_data(program, left_type)?;
            let right_data = structural_data(program, right_type)?;
            if left_data.symbol != right_data.symbol || left_data.name != right_data.name {
                return None;
            }
            let allow_direct_nested_mixed = left_path.is_empty()
                && right_path.is_empty()
                && matches!(
                    typed_trees::data::DataDefinition::shape_kind_from_members(
                        program.data_members(left_data)
                    ),
                    typed_trees::data::DataShapeKind::Record
                );
            let mut comparisons = Vec::new();
            collect_comparisons(
                program,
                left_parameter,
                right_parameter,
                left_type,
                &mut left_path,
                &mut right_path,
                &mut comparisons,
                &mut Vec::new(),
                allow_direct_nested_mixed,
            )?;
            let mut comparisons = comparisons.into_iter();
            let Some(first) = comparisons.next() else {
                return Some(CheckedBooleanExpression::Constant(true));
            };
            Some(
                comparisons.fold(first, |left, right| CheckedBooleanExpression::And {
                    left: Box::new(left),
                    right: Box::new(right),
                }),
            )
        }

        fn lower_structural_integer_expression(
            program: &TypedTrees,
            operators: &CheckedOperatorFacts,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let mut path = Vec::new();
            if let Some(parameter_position) =
                structural_parameter_field_path(program, parameters, expression, &mut path)
                && !path.is_empty()
                && let Some(type_reference) =
                    path_type_reference(program, parameters, parameter_position, &path)
                && let Some(primitive_type) = program.primitive_type_reference(type_reference)
                && is_integer(primitive_type)
                && primitive_type != PrimitiveType::Addr
            {
                return Some((
                    CheckedScalarExpression::StructuralParameterField {
                        parameter_position,
                        path,
                        primitive_type,
                    },
                    program.arithmetic_domain_for_type_reference(type_reference),
                ));
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(name) => {
                    let source_position = parameter_position(program, name, parameters)?;
                    let parameter = parameters.get(source_position)?;
                    let primitive_type =
                        program.primitive_type_reference(parameter.type_reference)?;
                    if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                        return None;
                    }
                    // Scalar values use the dense primitive namespace, while
                    // structural member roots above retain authored positions.
                    let position = parameters[..source_position]
                        .iter()
                        .filter(|parameter| {
                            program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                        })
                        .count();
                    Some((
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type,
                        },
                        program.arithmetic_domain_for_type_reference(parameter.type_reference),
                    ))
                }
                ExpressionNode::Integer(literal) => Some((
                    CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    },
                    literal
                        .landing()
                        .map(|landing| landing.domain)
                        .unwrap_or(ArithmeticDomain::Exact),
                )),
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::BitwiseNot
                        && operator_is_builtin(operators, expression) =>
                {
                    let (operand, domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        unary.operand,
                    )?;
                    let primitive_type = scalar_expression_type(&operand)?;
                    (is_integer(primitive_type) && primitive_type != PrimitiveType::Addr).then_some(
                        (
                            CheckedScalarExpression::IntegerBitwiseNot {
                                primitive_type,
                                operand: Box::new(operand),
                            },
                            domain,
                        ),
                    )
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::BitwiseAnd
                            | BinaryOperator::BitwiseOr
                            | BinaryOperator::BitwiseXor
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, right_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let domain = combine_arithmetic_domains(left_domain, right_domain)?;
                    let kind = match binary.operator {
                        BinaryOperator::BitwiseAnd => CheckedIntegerBinaryKind::BitwiseAnd,
                        BinaryOperator::BitwiseOr => CheckedIntegerBinaryKind::BitwiseOr,
                        BinaryOperator::BitwiseXor => CheckedIntegerBinaryKind::BitwiseXor,
                        _ => unreachable!("guarded structural bitwise operator"),
                    };
                    (is_integer(primitive_type)
                        && primitive_type != PrimitiveType::Addr
                        && scalar_expression_type(&right) == Some(primitive_type))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        domain,
                    ))
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                            | BinaryOperator::Modulo
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, right_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let domain = combine_arithmetic_domains(left_domain, right_domain)?;
                    let kind = checked_integer_binary_kind(binary.operator, domain)?;
                    let supported = matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract
                            | CheckedIntegerBinaryKind::ExactMultiply
                            | CheckedIntegerBinaryKind::ExactDivide
                            | CheckedIntegerBinaryKind::ExactRemainder
                    ) || (primitive_type != PrimitiveType::Addr
                        && matches!(
                            kind,
                            CheckedIntegerBinaryKind::WrappingAdd
                                | CheckedIntegerBinaryKind::SaturatingAdd
                                | CheckedIntegerBinaryKind::WrappingSubtract
                                | CheckedIntegerBinaryKind::SaturatingSubtract
                                | CheckedIntegerBinaryKind::WrappingMultiply
                                | CheckedIntegerBinaryKind::SaturatingMultiply
                                | CheckedIntegerBinaryKind::WrappingDivide
                                | CheckedIntegerBinaryKind::SaturatingDivide
                                | CheckedIntegerBinaryKind::WrappingRemainder
                                | CheckedIntegerBinaryKind::SaturatingRemainder
                        ));
                    (supported
                        && is_integer(primitive_type)
                        && scalar_expression_type(&right) == Some(primitive_type))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        domain,
                    ))
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, _) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let right_type = scalar_expression_type(&right)?;
                    let kind = checked_integer_binary_kind(binary.operator, left_domain)?;
                    (primitive_type != PrimitiveType::Addr
                        && right_type != PrimitiveType::Addr
                        && is_integer(primitive_type)
                        && is_integer(right_type)
                        && matches!(
                            kind,
                            CheckedIntegerBinaryKind::WrappingShiftLeft
                                | CheckedIntegerBinaryKind::WrappingShiftRight
                                | CheckedIntegerBinaryKind::ExactShiftLeft
                                | CheckedIntegerBinaryKind::ExactShiftRight
                        ))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        left_domain,
                    ))
                }
                _ => None,
            }
        }

        fn lower_structural_float_field(
            program: &TypedTrees,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<(PrimitiveType, CheckedStructuralParameterField)> {
            let mut path = Vec::new();
            let parameter_position =
                structural_parameter_field_path(program, parameters, expression, &mut path)?;
            if path.is_empty() {
                return None;
            }
            let primitive_type =
                path_primitive_type(program, parameters, parameter_position, &path)?;
            matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64).then_some((
                primitive_type,
                CheckedStructuralParameterField {
                    parameter_position,
                    path,
                },
            ))
        }

        fn lower_structural_case_membership(
            program: &TypedTrees,
            _operators: &CheckedOperatorFacts,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            let classifier = |candidate: ExpressionHandle| {
                let ExpressionNode::Name(name) = program.expression_table.expression(candidate)
                else {
                    return None;
                };
                program.data_definitions().iter().find_map(|data| {
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == name.symbol).then(|| {
                            (
                                data.symbol,
                                variant
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| variant.name.as_str().to_owned()),
                            )
                        })
                    })
                })
            };
            let (subject_expression, data_symbol, case) =
                if let Some((data, case)) = classifier(binary.right) {
                    (binary.left, data, case)
                } else if let Some((data, case)) = classifier(binary.left) {
                    (binary.right, data, case)
                } else {
                    return None;
                };
            let mut path = Vec::new();
            let parameter_position = structural_parameter_field_path(
                program,
                parameters,
                subject_expression,
                &mut path,
            )?;
            let subject_type = path_type_reference(program, parameters, parameter_position, &path)?;
            (structural_data(program, subject_type)?.symbol == data_symbol).then_some(
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: CheckedStructuralParameterField {
                        parameter_position,
                        path,
                    },
                    case,
                },
            )
        }

        if let Some(membership) =
            lower_structural_case_membership(program, operators, parameters, expression)
        {
            return Some(membership);
        }

        let mut path = Vec::new();
        if let Some(parameter_position) =
            structural_parameter_field_path(program, parameters, expression, &mut path)
            && !path.is_empty()
            && path_primitive_type(program, parameters, parameter_position, &path)
                == Some(PrimitiveType::Bool)
        {
            return Some(CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            });
        }

        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && let Some(equality) =
                lower_structural_equality(program, parameters, binary.left, binary.right)
        {
            return Some(if binary.operator == BinaryOperator::NotEqual {
                CheckedBooleanExpression::Not(Box::new(equality))
            } else {
                equality
            });
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Name(name) => {
                let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                    program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .iter()
                        .copied()
                        .find(|symbol| symbol.is_valid())
                });
                let name_text = program
                    .expression_table
                    .name_path_members(name.members)
                    .last();
                let source_position = parameters.iter().position(|parameter| {
                    name_symbol.is_some_and(|symbol| parameter.symbol == symbol)
                        || name_text.is_some_and(|text| parameter.name == *text)
                })?;
                let parameter = parameters.get(source_position)?;
                if parameter.is_mutable {
                    return (crate::values::mutable_scalar_parameter_type(program, parameter)
                        == Some(PrimitiveType::Bool)
                        && name.symbol == parameter.symbol
                        && name.head_symbol == parameter.symbol)
                        .then_some(CheckedBooleanExpression::StorageRead {
                            symbol: parameter.symbol,
                        });
                }
                (program.primitive_type_reference(parameter.type_reference)
                    == Some(PrimitiveType::Bool))
                .then(|| CheckedBooleanExpression::Parameter {
                    position: parameters[..source_position]
                        .iter()
                        .filter(|parameter| {
                            program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                        })
                        .count(),
                })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    lower_structural_boolean_expression(
                        program,
                        operators,
                        parameters,
                        unary.operand,
                    )?,
                )))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) && operator_is_builtin(operators, expression) =>
            {
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && let Some((primitive_type, left)) =
                    lower_structural_float_field(program, parameters, binary.left)
                    && let Some((right_type, right)) =
                        lower_structural_float_field(program, parameters, binary.right)
                    && primitive_type == right_type
                {
                    let (mut left, mut right) = (left, right);
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    let comparison = CheckedBooleanExpression::IeeeFloatComparison {
                        kind: if binary.operator == BinaryOperator::Equal {
                            CheckedIeeeFloatComparisonKind::Equal
                        } else {
                            CheckedIeeeFloatComparisonKind::NotEqual
                        },
                        primitive_type,
                        left,
                        right,
                    };
                    return Some(comparison);
                }
                let integer_operands = (|| {
                    let left = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    );
                    let right = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    );
                    let contextual_literal = |expression: ExpressionHandle,
                                              primitive_type: PrimitiveType|
                     -> Option<(
                        CheckedScalarExpression,
                        ArithmeticDomain,
                    )> {
                        let ExpressionNode::Integer(literal) =
                            program.expression_table.expression(expression)
                        else {
                            return None;
                        };
                        literal.landing().is_none().then(|| {
                            (
                                CheckedScalarExpression::IntegerLiteral {
                                    literal: literal.with_landing(IntegerLanding {
                                        landed_type: landed_for_primitive(primitive_type)
                                            .expect("fixed integer comparison context lands"),
                                        domain: ArithmeticDomain::Exact,
                                    }),
                                },
                                ArithmeticDomain::Exact,
                            )
                        })
                    };
                    let mut left = left?;
                    let mut right = right?;
                    match (
                        scalar_expression_type(&left.0),
                        scalar_expression_type(&right.0),
                    ) {
                        (None, Some(primitive_type)) => {
                            left = contextual_literal(binary.left, primitive_type)?;
                        }
                        (Some(primitive_type), None) => {
                            right = contextual_literal(binary.right, primitive_type)?;
                        }
                        (Some(_), Some(_)) => {}
                        (None, None) => return None,
                    }
                    let left = left.0;
                    let right = right.0;
                    let left_type = scalar_expression_type(&left)?;
                    (is_integer(left_type) && scalar_expression_type(&right)? == left_type)
                        .then_some((left, right))
                })();
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) || integer_operands.is_some()
                {
                    let (mut left, mut right) = integer_operands?;
                    let (kind, negated) = match binary.operator {
                        BinaryOperator::Equal => (CheckedIntegerComparisonKind::Equal, false),
                        BinaryOperator::NotEqual => (CheckedIntegerComparisonKind::Equal, true),
                        BinaryOperator::Less => (CheckedIntegerComparisonKind::LessThan, false),
                        BinaryOperator::LessOrEqual => {
                            (CheckedIntegerComparisonKind::LessOrEqual, false)
                        }
                        BinaryOperator::Greater => {
                            std::mem::swap(&mut left, &mut right);
                            (CheckedIntegerComparisonKind::LessThan, false)
                        }
                        BinaryOperator::GreaterOrEqual => {
                            std::mem::swap(&mut left, &mut right);
                            (CheckedIntegerComparisonKind::LessOrEqual, false)
                        }
                        _ => return None,
                    };
                    let comparison = CheckedBooleanExpression::IntegerComparison {
                        kind,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                    return Some(if negated {
                        CheckedBooleanExpression::Not(Box::new(comparison))
                    } else {
                        comparison
                    });
                }
                let equality = CheckedBooleanExpression::Equal {
                    left: Box::new(lower_structural_boolean_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?),
                    right: Box::new(lower_structural_boolean_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?),
                };
                Some(if binary.operator == BinaryOperator::NotEqual {
                    CheckedBooleanExpression::Not(Box::new(equality))
                } else {
                    equality
                })
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    && operator_is_builtin(operators, expression) =>
            {
                let left = Box::new(lower_structural_boolean_expression(
                    program,
                    operators,
                    parameters,
                    binary.left,
                )?);
                let right = Box::new(lower_structural_boolean_expression(
                    program,
                    operators,
                    parameters,
                    binary.right,
                )?);
                Some(if binary.operator == BinaryOperator::And {
                    CheckedBooleanExpression::And { left, right }
                } else {
                    CheckedBooleanExpression::Or { left, right }
                })
            }
            _ => None,
        }
    }

    if let Some(structural) =
        lower_structural_boolean_expression(program, operators, parameters, expression)
    {
        return Some(structural);
    }
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    lower_boolean_expression(
        program,
        operators,
        expression,
        parameters,
        &parameter_types,
        &[],
        exact_integer_casts,
    )
}

/// Lower one call argument in the caller state's checked scalar namespace.
/// Only the immutable scalar-prefix shape accepted by terminal scalar lowering
/// is represented; any wider state shape stays explicit as `None` so crash
/// refinement cannot claim a portable predicate it cannot later materialize.
pub(crate) fn lower_state_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    let parameters = program.state_parameters(state);
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..before_statement)?;
    let mut locals = Vec::new();
    for statement in prefix {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable || !local.initial_value.is_valid() {
            return None;
        }
        let primitive_type = program.primitive_type_reference(local.type_reference)?;
        locals.push(ScalarLocal {
            is_mutable: false,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
        });
    }
    lower_return_expression(
        program,
        operators,
        expression,
        parameters,
        &parameter_types,
        &locals,
        expected_type,
        exact_integer_casts,
    )
}

/// Lower one scalar argument inside a structural/Unit state. Structural
/// parameters retain their separate custody namespace; only primitive
/// parameters and earlier immutable primitive locals occupy scalar positions.
pub(crate) fn lower_unit_scalar_argument(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..before_statement)?;
    let mut locals = Vec::new();
    for statement in prefix {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable || !local.initial_value.is_valid() {
            return None;
        }
        let primitive_type = program.primitive_type_reference(local.type_reference)?;
        locals.push(ScalarLocal {
            is_mutable: false,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
        });
    }
    lower_return_expression(
        program,
        operators,
        expression,
        &parameters,
        &parameter_types,
        &locals,
        expected_type,
        &[],
    )
}

fn lower_return_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    result_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    if let Some(value) =
        land_anonymous_scalar_expression(program, operators, expression, result_type)
    {
        return Some(value);
    }
    if result_type == PrimitiveType::Bool {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
        .map(|expression| CheckedScalarExpression::Boolean(Box::new(expression)));
    }
    if let ExpressionNode::Float(literal) = program.expression_table.expression(expression) {
        let value = match (result_type, literal.landing()) {
            (PrimitiveType::F32, Some(numerics::literals::FloatFormat::F32)) => {
                semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
            }
            (PrimitiveType::F64, Some(numerics::literals::FloatFormat::F64)) => {
                semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
            }
            _ => return None,
        };
        return Some(CheckedScalarExpression::IeeeFloatLiteral { value });
    }
    let (expression, _) = lower_scalar_expression(
        program,
        operators,
        expression,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    match scalar_expression_type(&expression) {
        Some(actual_type) => (actual_type == result_type).then_some(expression),
        None => land_contextual_integer_literal(expression, result_type),
    }
}

fn land_anonymous_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    validation::land_anonymous_integer_expression(program, expression, destination, |expression| {
        match operators.expression_use(expression) {
            Some(operator) => operator.status == CheckedOperatorResolutionStatus::BuiltinFallback,
            None => validation::has_anonymous_operator_meaning(program, expression),
        }
    })
    .map(|literal| CheckedScalarExpression::IntegerLiteral { literal })
}

fn lower_scalar_operands(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    binary: &typed_trees::expression::TableBinaryExpression,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(
    (CheckedScalarExpression, ArithmeticDomain),
    (CheckedScalarExpression, ArithmeticDomain),
)> {
    let lower = |expression| {
        lower_scalar_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    };
    let mut left = lower(binary.left);
    let mut right = lower(binary.right);
    // Only the actual peer operand supplies a carrier. A wholly anonymous
    // subtree is evaluated exactly before landing; typed operations and calls
    // are rejected by this query and retain their original semantics.
    if let Some(destination) = right
        .as_ref()
        .and_then(|(value, _)| scalar_expression_type(value))
        && let Some(value) =
            land_anonymous_scalar_expression(program, operators, binary.left, destination)
    {
        left = Some((value, ArithmeticDomain::Exact));
    }
    if let Some(destination) = left
        .as_ref()
        .and_then(|(value, _)| scalar_expression_type(value))
        && let Some(value) =
            land_anonymous_scalar_expression(program, operators, binary.right, destination)
    {
        right = Some((value, ArithmeticDomain::Exact));
    }
    Some((left?, right?))
}

fn lower_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    if let Some(length) = exact_inline_literal_subslice_length(program, expression) {
        return Some((length, ArithmeticDomain::Exact));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
                let parameter = &parameters[position];
                if parameter.is_mutable {
                    let primitive_type =
                        crate::values::mutable_scalar_parameter_type(program, parameter)?;
                    if path.symbol != parameter.symbol || path.head_symbol != parameter.symbol {
                        return None;
                    }
                    return Some((
                        CheckedScalarExpression::StorageRead {
                            symbol: parameter.symbol,
                            primitive_type,
                        },
                        program.arithmetic_domain_for_type_reference(parameter.type_reference),
                    ));
                }
                return Some((
                    CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: parameter_types[position],
                    },
                    program
                        .arithmetic_domain_for_type_reference(parameters[position].type_reference),
                ));
            }
            let local_position = local_position(program, expression, path, locals)?;
            let local = &locals[local_position];
            if local.is_mutable {
                return Some((
                    CheckedScalarExpression::StorageRead {
                        symbol: local.symbol,
                        primitive_type: local.primitive_type,
                    },
                    local.arithmetic_domain,
                ));
            }
            let position = parameters.len().checked_add(
                locals[..local_position]
                    .iter()
                    .filter(|local| !local.is_mutable)
                    .count(),
            )?;
            Some((
                CheckedScalarExpression::Local {
                    position,
                    primitive_type: locals[local_position].primitive_type,
                },
                locals[local_position].arithmetic_domain,
            ))
        }
        ExpressionNode::Integer(literal) => Some((
            CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone(),
            },
            literal
                .landing()
                .map(|landing| landing.domain)
                .unwrap_or(ArithmeticDomain::Exact),
        )),
        ExpressionNode::Cast(cast) if !cast.form.is_recast() && cast.semantic_domain.is_empty() => {
            let target_type = program.primitive_type_reference(cast.target_type)?;
            if !is_integer(target_type) {
                return None;
            }
            let operand =
                land_anonymous_scalar_expression(program, operators, cast.value, target_type)
                    .or_else(|| {
                        lower_scalar_expression(
                            program,
                            operators,
                            cast.value,
                            parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                        )
                        .map(|(value, _)| value)
                    })?;
            construct_integer_cast(program, expression, operand, exact_integer_casts)
        }
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::BitwiseNot
                && operator_is_builtin(operators, expression) =>
        {
            let (operand, domain) = lower_scalar_expression(
                program,
                operators,
                unary.operand,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            construct_integer_bitwise_not(operand, domain)
        }
        ExpressionNode::Binary(binary) if operator_is_builtin(operators, expression) => {
            let ((left, left_domain), (right, right_domain)) = lower_scalar_operands(
                program,
                operators,
                binary,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            construct_integer_binary(binary.operator, left, left_domain, right, right_domain)
        }
        _ => None,
    }
}

/// Construct one selected builtin operation from operands whose evaluation
/// order has already been retained by either the pure tree or computation plan.
fn construct_integer_binary(
    operator: BinaryOperator,
    mut left: CheckedScalarExpression,
    left_domain: ArithmeticDomain,
    mut right: CheckedScalarExpression,
    right_domain: ArithmeticDomain,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let shift = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    let domain = if shift {
        left_domain
    } else {
        combine_arithmetic_domains(left_domain, right_domain)?
    };
    let kind = checked_integer_binary_kind(operator, domain)?;
    let mut left_type = scalar_expression_type(&left);
    let mut right_type = scalar_expression_type(&right);
    // Unary `-value` is parsed as a compiler-generated anonymous
    // `0 - value`. That zero has no parse-site suffix from which the
    // ordinary literal stamper can learn a carrier, so retain the
    // binary expression's already-checked operand carrier here. This
    // is contextual literal landing, not a new negation meaning.
    if operator == BinaryOperator::Subtract && left_type.is_none() {
        left = land_anonymous_zero(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if left_type.is_none() {
        left = land_contextual_integer_literal(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if right_type.is_none() {
        right = land_contextual_integer_literal(right, left_type?)?;
        right_type = scalar_expression_type(&right);
    }
    let primitive_type = left_type?;
    let right_type = right_type?;
    if !is_integer(primitive_type)
        || !is_integer(right_type)
        || (!shift && right_type != primitive_type)
    {
        return None;
    }
    Some((
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left: Box::new(left),
            right: Box::new(right),
        },
        domain,
    ))
}

fn construct_integer_bitwise_not(
    operand: CheckedScalarExpression,
    domain: ArithmeticDomain,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let primitive_type = scalar_expression_type(&operand)?;
    is_integer(primitive_type).then_some((
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand: Box::new(operand),
        },
        domain,
    ))
}

/// Retain cast meaning and any partial-conversion proof at the original source
/// occurrence even when the operand is a completed computation-plan value.
fn construct_integer_cast(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operand: CheckedScalarExpression,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let target_type = program.primitive_type_reference(cast.target_type)?;
    if !is_integer(target_type) {
        return None;
    }
    let source_type = scalar_expression_type(&operand);
    if source_type.is_none()
        && cast.domain == ArithmeticDomain::Exact
        && target_type != PrimitiveType::Addr
        && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
    {
        return Some((literal, cast.domain));
    }
    let source_type = source_type?;
    if source_type == target_type {
        return Some((operand, cast.domain));
    }
    // A compile-known exact conversion does not need a runtime cast
    // operation or a carried flow assumption: validation has already
    // proved the spelling denotes a target value, and the checked
    // carrier can retain that value directly at its new landing. Keep
    // address conversions out of this fixed-integer slice; addr is a
    // distinct carrier even when its current representation is u64.
    if cast.domain == ArithmeticDomain::Exact
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
        && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
    {
        return Some((literal, cast.domain));
    }
    // A full-carrier inclusion needs no occurrence proof. Preserve it
    // as widening even when validation also retained a bounded range
    // for this spelling; exact-cast obligations are only necessary for
    // partial fixed-integer conversions.
    if integer_widen_is_total(source_type, target_type) {
        return Some((
            CheckedScalarExpression::IntegerWiden {
                primitive_type: target_type,
                operand: Box::new(operand),
            },
            cast.domain,
        ));
    }
    if cast.domain == ArithmeticDomain::Exact
        && let Some(fact) = exact_integer_casts
            .iter()
            .find(|fact| fact.expression == expression)
        && fact.source_type == source_type
        && fact.target_type == target_type
    {
        return Some((
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: target_type,
                operand: Box::new(operand),
                range: CheckedIntegerRange {
                    minimum: fact.minimum.clone(),
                    maximum: fact.maximum.clone(),
                },
            },
            cast.domain,
        ));
    }
    // All remaining cast shapes fail closed at this source-independent
    // boundary: no total conversion and no retained occurrence proof.
    None
}

/// Reclose the one source-level slice view whose length is already fixed by
/// two literal bounds. This is a value fact, not a backend fold: retaining the
/// landed `u64` here lets later checked control replay the exact initializer
/// without consulting a target representation.
fn exact_inline_literal_subslice_length(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(member.receiver)
    else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
        return None;
    }
    let ExpressionNode::Integer(start) = program.expression_table.expression(range.start) else {
        return None;
    };
    let ExpressionNode::Integer(end) = program.expression_table.expression(range.end) else {
        return None;
    };
    let length = end.value_u64()?.checked_sub(start.value_u64()?)?;
    let length = i64::try_from(length).ok()?;
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(length).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            },
        ),
    })
}

fn land_anonymous_zero(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() || literal.value_i64() != Some(0) {
        return None;
    }
    let landed_type = match primitive_type {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr => LandedIntegerType::Addr,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

fn retag_exact_integer_literal(
    expression: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let landed_type = landed_for_primitive(primitive_type)?;
    let fits = if landed_type.is_signed() {
        let value = literal.value_i64()?;
        let bits = landed_type.bit_width();
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        let value = i128::from(value);
        minimum <= value && value <= maximum
    } else {
        let value = literal.value_u64()?;
        let bits = landed_type.bit_width();
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        value <= maximum
    };
    fits.then(|| CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

fn land_contextual_integer_literal(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() {
        return None;
    }
    let landed_type = match primitive_type {
        PrimitiveType::Addr => LandedIntegerType::Addr,
        primitive_type => landed_for_primitive(primitive_type)?,
    };
    let fits = if landed_type.is_signed() {
        let value = i128::from(literal.value_i64()?);
        let bits = landed_type.bit_width();
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        minimum <= value && value <= maximum
    } else {
        let value = literal.value_u64()?;
        let bits = landed_type.bit_width();
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        value <= maximum
    };
    fits.then(|| CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

fn landed_for_primitive(primitive_type: PrimitiveType) -> Option<LandedIntegerType> {
    Some(match primitive_type {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    })
}

fn lower_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
                let parameter = &parameters[position];
                if parameter.is_mutable {
                    return (crate::values::mutable_scalar_parameter_type(program, parameter)
                        == Some(PrimitiveType::Bool)
                        && path.symbol == parameter.symbol
                        && path.head_symbol == parameter.symbol)
                        .then_some(CheckedBooleanExpression::StorageRead {
                            symbol: parameter.symbol,
                        });
                }
                return (parameter_types[position] == PrimitiveType::Bool)
                    .then_some(CheckedBooleanExpression::Parameter { position });
            }
            let local_position = local_position(program, expression, path, locals)?;
            let local = &locals[local_position];
            if local.is_mutable {
                return (local.primitive_type == PrimitiveType::Bool).then_some(
                    CheckedBooleanExpression::StorageRead {
                        symbol: local.symbol,
                    },
                );
            }
            let position = parameters.len().checked_add(
                locals[..local_position]
                    .iter()
                    .filter(|local| !local.is_mutable)
                    .count(),
            )?;
            (locals[local_position].primitive_type == PrimitiveType::Bool)
                .then_some(CheckedBooleanExpression::Local { position })
        }
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && operator_is_builtin(operators, expression) =>
        {
            Some(CheckedBooleanExpression::Not(Box::new(
                lower_boolean_expression(
                    program,
                    operators,
                    unary.operand,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?,
            )))
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) && operator_is_builtin(operators, expression) =>
        {
            let integer_comparison = (|| {
                let ((left, _), (right, _)) = lower_scalar_operands(
                    program,
                    operators,
                    binary,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?;
                construct_integer_comparison(binary.operator, left, right)
            })();
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || integer_comparison.is_some()
            {
                return integer_comparison;
            }
            let equality = CheckedBooleanExpression::Equal {
                left: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?),
                right: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?),
            };
            Some(if binary.operator == BinaryOperator::NotEqual {
                CheckedBooleanExpression::Not(Box::new(equality))
            } else {
                equality
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && operator_is_builtin(operators, expression) =>
        {
            let left = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.left,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?);
            let right = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?);
            Some(if binary.operator == BinaryOperator::And {
                CheckedBooleanExpression::And { left, right }
            } else {
                CheckedBooleanExpression::Or { left, right }
            })
        }
        _ => None,
    }
}

/// Comparison normalization may swap completed values for `>` and `>=`; it
/// does not change the source operand evaluation order retained by the caller.
fn construct_integer_comparison(
    operator: BinaryOperator,
    mut left: CheckedScalarExpression,
    mut right: CheckedScalarExpression,
) -> Option<CheckedBooleanExpression> {
    match (
        scalar_expression_type(&left),
        scalar_expression_type(&right),
    ) {
        (Some(primitive_type), None) => {
            right = land_contextual_integer_literal(right, primitive_type)?;
        }
        (None, Some(primitive_type)) => {
            left = land_contextual_integer_literal(left, primitive_type)?;
        }
        (Some(_), Some(_)) => {}
        (None, None) => return None,
    }
    let left_type = scalar_expression_type(&left)?;
    if !is_integer(left_type) || scalar_expression_type(&right)? != left_type {
        return None;
    }
    let (kind, negated) = match operator {
        BinaryOperator::Equal => (CheckedIntegerComparisonKind::Equal, false),
        BinaryOperator::NotEqual => (CheckedIntegerComparisonKind::Equal, true),
        BinaryOperator::Less => (CheckedIntegerComparisonKind::LessThan, false),
        BinaryOperator::LessOrEqual => (CheckedIntegerComparisonKind::LessOrEqual, false),
        BinaryOperator::Greater => {
            std::mem::swap(&mut left, &mut right);
            (CheckedIntegerComparisonKind::LessThan, false)
        }
        BinaryOperator::GreaterOrEqual => {
            std::mem::swap(&mut left, &mut right);
            (CheckedIntegerComparisonKind::LessOrEqual, false)
        }
        _ => return None,
    };
    let comparison = CheckedBooleanExpression::IntegerComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    };
    Some(if negated {
        CheckedBooleanExpression::Not(Box::new(comparison))
    } else {
        comparison
    })
}

fn lower_boolean_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
    };
    if binary.operator == BinaryOperator::Equal && operator_is_builtin(operators, expression) {
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(true), _) => {
                return lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                );
            }
            (_, ExpressionNode::Boolean(true)) => {
                return lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                );
            }
            _ => {}
        }
    }
    lower_boolean_expression(
        program,
        operators,
        expression,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )
}

fn parameter_position(
    program: &TypedTrees,
    path: &typed_trees::expression::TableNamePath,
    parameters: &[StateParameter],
) -> Option<usize> {
    let members = program.expression_table.name_path_members(path.members);
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (members.len() == 1)
        .then(|| {
            parameters.iter().position(|parameter| {
                if path.symbol.is_valid() {
                    parameter.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    parameter.symbol == path.head_symbol
                } else {
                    parameter.name.as_str() == members[0].as_str()
                }
            })
        })
        .flatten()
}

fn local_position(
    program: &TypedTrees,
    expression: ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
    locals: &[ScalarLocal],
) -> Option<usize> {
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            locals.iter().rposition(|local| {
                if local.is_mutable {
                    // Bare storage reads require both retained resolved identities.
                    // Spelling cannot repair a missing or stale storage handle.
                    local.symbol.is_valid()
                        && local.symbol == path.symbol
                        && local.symbol == path.head_symbol
                } else if path.symbol.is_valid() {
                    local.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    local.symbol == path.head_symbol
                } else {
                    local.name == program.expression_table.display_name(expression)
                }
            })
        })
        .flatten()
}

pub(crate) fn scalar_expression_type(
    expression: &CheckedScalarExpression,
) -> Option<PrimitiveType> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::StorageRead { primitive_type, .. }
        | CheckedScalarExpression::Local { primitive_type, .. }
        | CheckedScalarExpression::StructuralParameterField { primitive_type, .. }
        | CheckedScalarExpression::IntegerBinary { primitive_type, .. }
        | CheckedScalarExpression::IntegerBitwiseNot { primitive_type, .. }
        | CheckedScalarExpression::IntegerWiden { primitive_type, .. }
        | CheckedScalarExpression::IntegerExactCast { primitive_type, .. } => Some(*primitive_type),
        CheckedScalarExpression::IntegerLiteral { literal } => {
            primitive_for_landed(literal.landing()?.landed_type)
        }
        CheckedScalarExpression::IeeeFloatLiteral { value } => Some(match value {
            semantic_vocabulary::IeeeFloatValue::Binary32(_) => PrimitiveType::F32,
            semantic_vocabulary::IeeeFloatValue::Binary64(_) => PrimitiveType::F64,
        }),
        CheckedScalarExpression::Boolean(_) => Some(PrimitiveType::Bool),
    }
}

fn primitive_for_landed(landed: numerics::literals::LandedIntegerType) -> Option<PrimitiveType> {
    use numerics::literals::LandedIntegerType;
    Some(match landed {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    })
}

fn is_integer(primitive: PrimitiveType) -> bool {
    !matches!(
        primitive,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
    )
}

fn integer_widen_is_total(source: PrimitiveType, target: PrimitiveType) -> bool {
    fn shape(primitive: PrimitiveType) -> Option<(bool, u8)> {
        Some(match primitive {
            PrimitiveType::I8 => (true, 8),
            PrimitiveType::I16 => (true, 16),
            PrimitiveType::I32 => (true, 32),
            PrimitiveType::I64 => (true, 64),
            PrimitiveType::U8 => (false, 8),
            PrimitiveType::U16 => (false, 16),
            PrimitiveType::U32 => (false, 32),
            PrimitiveType::U64 => (false, 64),
            PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
                return None;
            }
        })
    }
    let Some((source_signed, source_bits)) = shape(source) else {
        return false;
    };
    let Some((target_signed, target_bits)) = shape(target) else {
        return false;
    };
    source_bits < target_bits && (!source_signed || target_signed)
}

fn operator_is_builtin(operators: &CheckedOperatorFacts, expression: ExpressionHandle) -> bool {
    operators
        .expression_use(expression)
        .is_none_or(|operator_use| {
            operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
        })
}

fn combine_arithmetic_domains(
    left: ArithmeticDomain,
    right: ArithmeticDomain,
) -> Option<ArithmeticDomain> {
    match (left, right) {
        (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => Some(domain),
        (left, right) if left == right => Some(left),
        _ => None,
    }
}

fn checked_integer_binary_kind(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
) -> Option<CheckedIntegerBinaryKind> {
    match (operator, domain) {
        (BinaryOperator::BitwiseAnd, _) => Some(CheckedIntegerBinaryKind::BitwiseAnd),
        (BinaryOperator::BitwiseOr, _) => Some(CheckedIntegerBinaryKind::BitwiseOr),
        (BinaryOperator::BitwiseXor, _) => Some(CheckedIntegerBinaryKind::BitwiseXor),
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftRight)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftRight)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftLeft)
        }
        (BinaryOperator::Add, ArithmeticDomain::Exact) => Some(CheckedIntegerBinaryKind::ExactAdd),
        (BinaryOperator::Subtract, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactMultiply)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingRemainder)
        }
        (BinaryOperator::Add, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingAdd)
        }
        (BinaryOperator::Add, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingAdd)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingSubtract)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingMultiply)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingMultiply)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::Arena;

    #[test]
    fn boolean_guard_selection_preserves_both_polarities_and_operator_meaning() {
        for value in [false, true] {
            let mut program = TypedTrees::default();
            let left = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let right = program
                .expression_table
                .insert(ExpressionNode::Boolean(value));
            let expression = program.expression_table.insert(ExpressionNode::Binary(
                typed_trees::expression::TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                },
            ));
            for (status, accepted) in [
                (CheckedOperatorResolutionStatus::BuiltinFallback, true),
                (CheckedOperatorResolutionStatus::Resolved, false),
                (CheckedOperatorResolutionStatus::Missing, false),
                (CheckedOperatorResolutionStatus::Ambiguous, false),
            ] {
                let mut uses = Arena::new();
                uses.append(checked_trees::CheckedOperatorUseFact {
                    expression,
                    status,
                    ..Default::default()
                });
                let operators = CheckedOperatorFacts::with_roots(uses, Arena::new(), Arena::new());
                assert_eq!(
                    lower_boolean_guard(&program, &operators, expression, &[], &[], &[], &[])
                        .is_some(),
                    accepted,
                    "value={value}, status={status:?}",
                );
            }
        }
    }

    #[test]
    fn retained_widening_requires_complete_fixed_integer_range_containment() {
        assert!(integer_widen_is_total(
            PrimitiveType::U8,
            PrimitiveType::U64
        ));
        assert!(integer_widen_is_total(
            PrimitiveType::I8,
            PrimitiveType::I64
        ));
        assert!(integer_widen_is_total(
            PrimitiveType::U8,
            PrimitiveType::I16
        ));
        assert!(!integer_widen_is_total(
            PrimitiveType::I8,
            PrimitiveType::U16
        ));
        assert!(!integer_widen_is_total(
            PrimitiveType::U16,
            PrimitiveType::U8
        ));
        assert!(!integer_widen_is_total(
            PrimitiveType::U32,
            PrimitiveType::Addr
        ));
    }

    #[test]
    fn compile_known_exact_integer_conversion_relands_only_representable_fixed_values() {
        let source = CheckedScalarExpression::IntegerLiteral {
            literal: numerics::literals::IntegerLiteral::from_value(127).with_landing(
                IntegerLanding {
                    landed_type: LandedIntegerType::I64,
                    domain: ArithmeticDomain::Exact,
                },
            ),
        };
        let narrowed = retag_exact_integer_literal(&source, PrimitiveType::I8)
            .expect("127 is exactly representable as i8");
        assert_eq!(scalar_expression_type(&narrowed), Some(PrimitiveType::I8));
        assert!(retag_exact_integer_literal(&source, PrimitiveType::Addr).is_none());

        let outside = CheckedScalarExpression::IntegerLiteral {
            literal: numerics::literals::IntegerLiteral::from_value(128).with_landing(
                IntegerLanding {
                    landed_type: LandedIntegerType::I64,
                    domain: ArithmeticDomain::Exact,
                },
            ),
        };
        assert!(retag_exact_integer_literal(&outside, PrimitiveType::I8).is_none());
    }

    #[test]
    fn compile_known_exact_integer_conversion_lands_untyped_literals() {
        let source = CheckedScalarExpression::IntegerLiteral {
            literal: numerics::literals::IntegerLiteral::from_value(70),
        };
        let landed = retag_exact_integer_literal(&source, PrimitiveType::I32)
            .expect("70 is exactly representable as i32");
        assert_eq!(scalar_expression_type(&landed), Some(PrimitiveType::I32));
        assert!(retag_exact_integer_literal(&source, PrimitiveType::Addr).is_none());
    }
}
