use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedIntegerRange, CheckedLocatedScalarExpression,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
    CheckedScalarExpressionPlans, CheckedScalarExpressionRole, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
};
use psi_numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, LandedIntegerType},
};
use psi_typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    signature::StateParameter,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Debug, Clone)]
struct ScalarLocal {
    symbol: psi_symbols::SymbolHandle,
    name: String,
    primitive_type: PrimitiveType,
    arithmetic_domain: ArithmeticDomain,
}

pub(crate) fn build_checked_scalar_expression_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> CheckedScalarExpressionPlans {
    let mut expressions = Vec::new();
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
                    StatementNode::LocalData(local)
                        if !local.is_mutable && local.initial_value.is_valid() =>
                    {
                        let Some(primitive_type) =
                            program.primitive_type_reference(local.type_reference)
                        else {
                            continue;
                        };
                        let binding_ordinal = u32::try_from(locals.len()).ok();
                        if let Some(binding_ordinal) = binding_ordinal {
                            if let ExpressionNode::Call(call) =
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
                                )
                            {
                                expressions.extend(arguments);
                            }
                            if let Some(arguments) = lower_direct_call_binding_arguments(
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
                            ) {
                                expressions.extend(arguments);
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
                                expressions.push(CheckedLocatedScalarExpression {
                                    state: state.symbol,
                                    statement_ordinal,
                                    role: CheckedScalarExpressionRole::LocalInitializer {
                                        binding_ordinal,
                                    },
                                    expression: initializer,
                                });
                            }
                        }
                        locals.push(ScalarLocal {
                            symbol: local.symbol,
                            name: local.name.as_str().to_owned(),
                            primitive_type,
                            arithmetic_domain: program
                                .arithmetic_domain_for_type_reference(local.type_reference),
                        });
                    }
                    StatementNode::Expression(expression) => {
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
                            )
                        {
                            expressions.extend(arguments);
                        }
                        if let Some(result_type) = result_type
                            && let Some(expression) = lower_return_expression(
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
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Return,
                                expression,
                            });
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        // Assignment values already have general checked-value
                        // custody. Retain the exact scalar spelling separately
                        // so structural effect planning never has to revisit a
                        // typed expression handle. This first consumer needs
                        // only direct primitive literals; wider assignment
                        // expressions remain outside its admitted vocabulary.
                        if !matches!(
                            program.expression_table.expression(assignment.value),
                            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                        ) {
                            continue;
                        }
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
                            &[],
                            &[],
                            &[],
                            target_type,
                            exact_integer_casts,
                        ) else {
                            continue;
                        };
                        let direct_literal =
                            matches!(expression, CheckedScalarExpression::IntegerLiteral { .. })
                                || matches!(
                                    &expression,
                                    CheckedScalarExpression::Boolean(boolean)
                                        if matches!(
                                            boolean.as_ref(),
                                            CheckedBooleanExpression::Constant(_)
                                        )
                                );
                        if !direct_literal {
                            continue;
                        }
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
                        ) {
                            expressions.extend(arguments);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard
                            && let Some(guard) = lower_positive_boolean_guard(
                                program,
                                operators,
                                guard,
                                &scalar_parameters,
                                &parameter_types,
                                &locals,
                                exact_integer_casts,
                            )
                            .or_else(|| {
                                lower_closed_integer_literal_guard(program, operators, guard)
                            })
                        {
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Guard,
                                expression: CheckedScalarExpression::Boolean(Box::new(guard)),
                            });
                        }
                        if transition.exit == psi_typed_trees::statement::TransitionExit::Ordinary
                            && transition.guard == TransitionGuardNode::Always
                            && !transition.continuation.is_valid()
                            && let TransitionTargetNode::Value(expression) =
                                program.statement_table.transition_target(transition.target)
                            && let Some(result_type) = result_type
                            && let Some(expression) = lower_return_expression(
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
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Return,
                                expression,
                            });
                        }
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(transition.target)
                        else {
                            continue;
                        };
                        let Some(target_state) = states
                            .iter()
                            .find(|candidate| candidate.symbol == path.symbol)
                        else {
                            continue;
                        };
                        let target_parameters = program.state_parameters(target_state);
                        for (argument_index, (argument, target_parameter)) in program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .zip(target_parameters)
                            .enumerate()
                        {
                            let Some(target_type) =
                                program.primitive_type_reference(target_parameter.type_reference)
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
                            let Ok(argument_ordinal) = u32::try_from(argument_index) else {
                                continue;
                            };
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::TransitionArgument {
                                    argument_ordinal,
                                },
                                expression,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    CheckedScalarExpressionPlans { expressions }
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

#[allow(clippy::too_many_arguments)]
fn lower_boundary_call_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &psi_typed_trees::state::State,
    statement_ordinal: u32,
    call_ordinal: usize,
    call_site: &crate::CallSite<'_>,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<Vec<CheckedLocatedScalarExpression>> {
    let target_symbol = match call_site {
        crate::CallSite::Statement(call) => call.target_symbol,
        crate::CallSite::Expression { call, .. } => call.target_symbol,
        crate::CallSite::TransitionNamed { .. } => return None,
    };
    let is_boundary = program.machines().iter().any(|machine| {
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
                .any(|signature| signature.symbol == target_symbol)
    });
    if !is_boundary {
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
        if target.is_self || target.is_const || target.is_mutable {
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
        output.push(CheckedLocatedScalarExpression {
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: u32::try_from(call_ordinal).ok()?,
                argument_ordinal: u32::try_from(scalar_index).ok()?,
            },
            expression: lowered?,
        });
        scalar_index = scalar_index.checked_add(1)?;
    }
    (explicit_index == explicit_arguments.len()).then_some(output)
}

#[allow(clippy::too_many_arguments)]
fn lower_direct_call_binding_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<Vec<CheckedLocatedScalarExpression>> {
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
    if target_parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
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
            Some(CheckedLocatedScalarExpression {
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
            })
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
    machine: &psi_typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
                let field_identity = |field: &psi_typed_trees::data::DataField| {
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned())
                };
                if let Some(case_name) = &member.case_variant {
                    let (case, field) = program.data_definitions().iter().find_map(|data| {
                        program.data_members(data).iter().find_map(|candidate| {
                            let psi_typed_trees::data::DataMember::Variant(variant) = candidate
                            else {
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
                                let psi_typed_trees::data::DataMember::Field(field) = candidate
                                else {
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
                            let psi_typed_trees::data::DataMember::Field(field) = member else {
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
            let Some(parameter) = usize::try_from(parameter_position)
                .ok()
                .and_then(|position| parameters.get(position))
            else {
                return None;
            };
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
                            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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

        fn structural_data<'program>(
            program: &'program TypedTrees,
            mut type_reference: TypeReferenceHandle,
        ) -> Option<&'program psi_typed_trees::data::DataDefinition> {
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

        fn structural_record_fields<'program>(
            program: &'program TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Vec<&'program psi_typed_trees::data::DataField>> {
            let data = structural_data(program, type_reference)?;
            let mut fields = Vec::new();
            for member in program.data_members(data) {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
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
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(members),
                psi_typed_trees::data::DataShapeKind::Enum
            ) {
                return None;
            }
            members
                .iter()
                .map(|member| {
                    let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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

        fn append_acyclic_structural_equality(
            program: &TypedTrees,
            left_parameter: u32,
            right_parameter: u32,
            type_reference: TypeReferenceHandle,
            left_path: &[CheckedStructuralPredicatePathSegment],
            right_path: &[CheckedStructuralPredicatePathSegment],
            comparisons: &mut Vec<CheckedBooleanExpression>,
            visiting: &mut Vec<psi_symbols::SymbolHandle>,
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
                match psi_typed_trees::data::DataDefinition::shape_kind_from_members(members) {
                    psi_typed_trees::data::DataShapeKind::Empty => Some(()),
                    psi_typed_trees::data::DataShapeKind::Record => {
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
                    psi_typed_trees::data::DataShapeKind::Enum => {
                        let mut arms = Vec::new();
                        for member in members {
                            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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
                    psi_typed_trees::data::DataShapeKind::Mixed => {
                        // The bounded nested mixed-shape slice permits exactly
                        // one, two, three, four, five, six, seven, or eight direct
                        // record fields before the sole mixed occurrence.
                        // Deeper records, case payloads, and two mixed siblings
                        // retain their fail-closed fence until their independent
                        // path and replay canaries land.
                        if !left_path.is_empty() || !right_path.is_empty() {
                            if !matches!(
                                (left_path, right_path),
                                (
                                    [CheckedStructuralPredicatePathSegment::Field(_)],
                                    [CheckedStructuralPredicatePathSegment::Field(_)]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                ) | (
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ],
                                    [
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_),
                                        CheckedStructuralPredicatePathSegment::Field(_)
                                    ]
                                )
                            ) || !allow_direct_nested_mixed
                                || *nested_mixed_seen
                            {
                                return None;
                            }
                            *nested_mixed_seen = true;
                        }
                        for member in members {
                            let psi_typed_trees::data::DataMember::Field(field) = member else {
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
                            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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
                left_path: &mut Vec<CheckedStructuralPredicatePathSegment>,
                right_path: &mut Vec<CheckedStructuralPredicatePathSegment>,
                output: &mut Vec<CheckedBooleanExpression>,
                visiting: &mut Vec<psi_symbols::SymbolHandle>,
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
                    psi_typed_trees::data::DataDefinition::shape_kind_from_members(
                        program.data_members(left_data)
                    ),
                    psi_typed_trees::data::DataShapeKind::Record
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
                        let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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
    state: &psi_typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
    state: &psi_typed_trees::state::State,
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
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
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

fn lower_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
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
            let position = parameters.len().checked_add(local_position)?;
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
            let (operand, _) = lower_scalar_expression(
                program,
                operators,
                cast.value,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
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
            let primitive_type = scalar_expression_type(&operand)?;
            is_integer(primitive_type).then_some((
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand: Box::new(operand),
                },
                domain,
            ))
        }
        ExpressionNode::Binary(binary) if operator_is_builtin(operators, expression) => {
            let (mut left, left_domain) = lower_scalar_expression(
                program,
                operators,
                binary.left,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            let (mut right, right_domain) = lower_scalar_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            let shift = matches!(
                binary.operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );
            let domain = if shift {
                left_domain
            } else {
                combine_arithmetic_domains(left_domain, right_domain)?
            };
            let kind = checked_integer_binary_kind(binary.operator, domain)?;
            let mut left_type = scalar_expression_type(&left);
            let mut right_type = scalar_expression_type(&right);
            // Unary `-value` is parsed as a compiler-generated anonymous
            // `0 - value`. That zero has no parse-site suffix from which the
            // ordinary literal stamper can learn a carrier, so retain the
            // binary expression's already-checked operand carrier here. This
            // is contextual literal landing, not a new negation meaning.
            if binary.operator == BinaryOperator::Subtract && left_type.is_none() {
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
        _ => None,
    }
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
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
                return (parameter_types[position] == PrimitiveType::Bool)
                    .then_some(CheckedBooleanExpression::Parameter { position });
            }
            let local_position = local_position(program, expression, path, locals)?;
            let position = parameters.len().checked_add(local_position)?;
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
            let integer_operands = (|| {
                let (left, _) = lower_scalar_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?;
                let (right, _) = lower_scalar_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?;
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

fn lower_positive_boolean_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
    if binary.operator == BinaryOperator::Equal {
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
    let guard = lower_boolean_expression(
        program,
        operators,
        expression,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    (is_integer_comparison(&guard) || contains_short_circuit(&guard)).then_some(guard)
}

fn parameter_position(
    program: &TypedTrees,
    path: &psi_typed_trees::expression::TableNamePath,
    parameters: &[StateParameter],
) -> Option<usize> {
    let members = program.expression_table.name_path_members(path.members);
    (members.len() == 1)
        .then(|| {
            parameters.iter().position(|parameter| {
                parameter.symbol == path.symbol
                    || parameter.symbol == path.head_symbol
                    || parameter.name.as_str() == members[0].as_str()
            })
        })
        .flatten()
}

fn local_position(
    program: &TypedTrees,
    expression: ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
    locals: &[ScalarLocal],
) -> Option<usize> {
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            locals.iter().rposition(|local| {
                (path.symbol.is_valid() && local.symbol == path.symbol)
                    || (path.head_symbol.is_valid() && local.symbol == path.head_symbol)
                    || local.name == program.expression_table.display_name(expression)
            })
        })
        .flatten()
}

pub(crate) fn scalar_expression_type(
    expression: &CheckedScalarExpression,
) -> Option<PrimitiveType> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::Local { primitive_type, .. }
        | CheckedScalarExpression::StructuralParameterField { primitive_type, .. }
        | CheckedScalarExpression::IntegerBinary { primitive_type, .. }
        | CheckedScalarExpression::IntegerBitwiseNot { primitive_type, .. }
        | CheckedScalarExpression::IntegerWiden { primitive_type, .. }
        | CheckedScalarExpression::IntegerExactCast { primitive_type, .. } => Some(*primitive_type),
        CheckedScalarExpression::IntegerLiteral { literal } => {
            primitive_for_landed(literal.landing()?.landed_type)
        }
        CheckedScalarExpression::Boolean(_) => Some(PrimitiveType::Bool),
    }
}

fn primitive_for_landed(
    landed: psi_numerics::literals::LandedIntegerType,
) -> Option<PrimitiveType> {
    use psi_numerics::literals::LandedIntegerType;
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

fn contains_short_circuit(expression: &CheckedBooleanExpression) -> bool {
    match expression {
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::Local { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IntegerComparison { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        CheckedBooleanExpression::Not(operand) => contains_short_circuit(operand),
        CheckedBooleanExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => true,
    }
}

fn is_integer_comparison(expression: &CheckedBooleanExpression) -> bool {
    match expression {
        CheckedBooleanExpression::IntegerComparison { .. } => true,
        CheckedBooleanExpression::Not(operand) => {
            matches!(
                operand.as_ref(),
                CheckedBooleanExpression::IntegerComparison { .. }
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            literal: psi_numerics::literals::IntegerLiteral::from_value(127).with_landing(
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
            literal: psi_numerics::literals::IntegerLiteral::from_value(128).with_landing(
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
            literal: psi_numerics::literals::IntegerLiteral::from_value(70),
        };
        let landed = retag_exact_integer_literal(&source, PrimitiveType::I32)
            .expect("70 is exactly representable as i32");
        assert_eq!(scalar_expression_type(&landed), Some(PrimitiveType::I32));
        assert!(retag_exact_integer_literal(&source, PrimitiveType::Addr).is_none());
    }
}
