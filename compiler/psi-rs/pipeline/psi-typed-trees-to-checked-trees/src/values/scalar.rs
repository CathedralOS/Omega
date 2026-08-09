use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedIntegerRange, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedScalarExpression, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole,
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
    types::PrimitiveType,
};

pub(crate) fn build_checked_scalar_expression_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
                            exact_integer_casts,
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
                                exact_integer_casts,
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

fn lower_return_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
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
        exact_integer_casts,
    )?;
    (scalar_expression_type(&expression)? == result_type).then_some(expression)
}

fn lower_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
                exact_integer_casts,
            )?;
            let source_type = scalar_expression_type(&operand)?;
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
                exact_integer_casts,
            )?;
            let (right, right_domain) = lower_scalar_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
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
            let right_type = scalar_expression_type(&right)?;
            // Unary `-value` is parsed as a compiler-generated anonymous
            // `0 - value`. That zero has no parse-site suffix from which the
            // ordinary literal stamper can learn a carrier, so retain the
            // binary expression's already-checked operand carrier here. This
            // is contextual literal landing, not a new negation meaning.
            if binary.operator == BinaryOperator::Subtract
                && scalar_expression_type(&left).is_none()
            {
                left = land_anonymous_zero(left, right_type)?;
            }
            let primitive_type = scalar_expression_type(&left)?;
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
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
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
                    exact_integer_casts,
                )?;
                let (right, _) = lower_scalar_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
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
                    exact_integer_casts,
                )?),
                right: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    parameter_types,
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
                exact_integer_casts,
            )?);
            let right = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.right,
                parameters,
                parameter_types,
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
    exact_integer_casts: &[psi_validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            parameter_types,
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
        exact_integer_casts,
    )?;
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
}
