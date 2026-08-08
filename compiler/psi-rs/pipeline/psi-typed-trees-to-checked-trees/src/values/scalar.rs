use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedLocatedScalarExpression, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedScalarExpression, CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
};
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    signature::StateParameter,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::PrimitiveType,
};

pub(crate) fn build_checked_scalar_expression_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
) -> CheckedScalarExpressionPlans {
    let mut expressions = Vec::new();
    for machine in program.machines() {
        let states = program.machine_states(machine);
        for state in states {
            let parameters = program.state_parameters(state);
            let Some(parameter_types) = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let Some(result_type) = program.primitive_type_reference(state.return_type) else {
                continue;
            };
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
                    StatementNode::Expression(expression) => {
                        if let Some(expression) = lower_return_expression(
                            program,
                            operators,
                            *expression,
                            parameters,
                            &parameter_types,
                            result_type,
                        ) {
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Return,
                                expression,
                            });
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard
                            && let Some(guard) = lower_positive_boolean_guard(
                                program,
                                operators,
                                guard,
                                parameters,
                                &parameter_types,
                            )
                        {
                            expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::Guard,
                                expression: CheckedScalarExpression::Boolean(Box::new(guard)),
                            });
                        }
                        let TransitionTargetNode::Named { path, arguments } =
                            program.statement_table.transition_target(transition.target)
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
                                parameters,
                                &parameter_types,
                                target_type,
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

fn lower_return_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    result_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    if result_type == PrimitiveType::Bool {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
        )
        .map(|expression| CheckedScalarExpression::Boolean(Box::new(expression)));
    }
    let (expression, _) =
        lower_scalar_expression(program, operators, expression, parameters, parameter_types)?;
    (scalar_expression_type(&expression)? == result_type).then_some(expression)
}

fn lower_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let position = parameter_position(program, path, parameters)?;
            Some((
                CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: parameter_types[position],
                },
                program.arithmetic_domain_for_type_reference(parameters[position].type_reference),
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
        ExpressionNode::Binary(binary) if operator_is_builtin(operators, expression) => {
            let (left, left_domain) = lower_scalar_expression(
                program,
                operators,
                binary.left,
                parameters,
                parameter_types,
            )?;
            let (right, right_domain) = lower_scalar_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
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
            let primitive_type = scalar_expression_type(&left)?;
            let right_type = scalar_expression_type(&right)?;
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

fn lower_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
) -> Option<CheckedBooleanExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
        ExpressionNode::Name(path) => {
            let position = parameter_position(program, path, parameters)?;
            (parameter_types[position] == PrimitiveType::Bool)
                .then_some(CheckedBooleanExpression::Parameter { position })
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
                )?;
                let (right, _) = lower_scalar_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
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
                )?),
                right: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
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
            )?);
            let right = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
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
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
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
                );
            }
            (_, ExpressionNode::Boolean(true)) => {
                return lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    parameter_types,
                );
            }
            _ => {}
        }
    }
    let guard =
        lower_boolean_expression(program, operators, expression, parameters, parameter_types)?;
    (is_integer_comparison(&guard) || contains_short_circuit(&guard)).then_some(guard)
}

fn parameter_position(
    program: &TypedTrees,
    path: &psi_typed_trees::expression::TableNamePath,
    parameters: &[StateParameter],
) -> Option<usize> {
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            parameters.iter().position(|parameter| {
                parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
            })
        })
        .flatten()
}

fn scalar_expression_type(expression: &CheckedScalarExpression) -> Option<PrimitiveType> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::IntegerBinary { primitive_type, .. } => Some(*primitive_type),
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
        | CheckedBooleanExpression::IntegerComparison { .. } => false,
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
