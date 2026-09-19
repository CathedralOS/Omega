//! Lowering Boolean expressions and guards, including closed evaluation and
//! integer comparisons.

use crate::values::scalar::case_membership;
use crate::values::scalar::constant_array_projection;
use crate::values::scalar::expression_facts::{
    is_integer, local_position, operator_is_builtin, parameter_position, scalar_expression_type,
};
use crate::values::scalar::expression_plans::{ScalarLocal, lower_closed_integer_literal_guard};
use crate::values::scalar::primitive_reference_read;
use crate::values::scalar::scalar_lowering::{
    land_contextual_integer_literal, lower_scalar_operands,
};
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Recover a normal-return Boolean using the same selected operations as
/// scalar computation folding, without assumptions about any runtime binding.
/// Callers still evaluate the expression's children: a known result (such as a
/// fixed extent) does not establish that evaluating its source is effect-free.
pub(crate) fn evaluate_closed_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<bool> {
    let selected = lower_boolean_expression(
        program,
        operators,
        expression,
        &[],
        &[],
        &[],
        &[],
        exact_integer_casts,
    )?;
    let facts::ScalarValue::Boolean(value) = crate::values::evaluate_checked_scalar(
        &CheckedScalarExpression::Boolean(Box::new(selected)),
        &mut |_| None,
    )?
    else {
        return None;
    };
    Some(value)
}

pub(crate) fn lower_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if let Some(CheckedScalarExpression::Boolean(read)) = primitive_reference_read::lower(
        program,
        authored_parameters,
        expression,
        PrimitiveType::Bool,
    ) {
        return Some(*read);
    }
    if let Some(membership) = case_membership::lower(program, authored_parameters, expression) {
        return Some(membership);
    }
    if let Some((leaf, PrimitiveType::Bool)) =
        validation::closed_record_scalar_projection(program, expression)
    {
        let ExpressionNode::Boolean(value) = program.expression_table.expression(leaf) else {
            return None;
        };
        return Some(CheckedBooleanExpression::Constant(*value));
    }
    if let Some(leaf) = constant_array_projection::selected_leaf(
        program,
        operators,
        authored_parameters,
        expression,
    ) {
        return lower_boolean_expression(
            program,
            operators,
            leaf,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) && let Some((CheckedScalarExpression::Boolean(field), _)) =
        structural_fields::lower_structural_parameter_field(
            program,
            authored_parameters,
            expression,
        )
    {
        return Some(*field);
    }
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
                    authored_parameters,
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
                    authored_parameters,
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
            // A bounded owned byte carrier compared against a byte-sequence
            // literal has no scalar operand pair: content equality is the
            // live-length observation plus each literal byte at its fixed
            // index, evaluated left to right so the length dominates every
            // indexed read.
            if let Some(equality) =
                bounded_carrier_literal_equality(program, binary, authored_parameters)
            {
                return Some(if binary.operator == BinaryOperator::NotEqual {
                    CheckedBooleanExpression::Not(Box::new(equality))
                } else {
                    equality
                });
            }
            let equality = CheckedBooleanExpression::Equal {
                left: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?),
                right: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    authored_parameters,
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
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?);
            let right = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.right,
                parameters,
                authored_parameters,
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

/// `carrier == "literal"` on a bounded owned byte field admits no scalar
/// operand pair: content equality is the live-length observation plus each
/// literal byte at its fixed index. The fold keeps the length comparison
/// leftmost so short-circuit evaluation dominates every indexed read.
fn bounded_carrier_literal_equality(
    program: &TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
    authored_parameters: &[StateParameter],
) -> Option<CheckedBooleanExpression> {
    for (carrier, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
        let ExpressionNode::String(bytes) = program.expression_table.expression(literal) else {
            continue;
        };
        let (parameter_position, path, type_reference) =
            structural_fields::structural_parameter_place(program, authored_parameters, carrier)?;
        if !matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(
                program,
                type_reference,
                &[],
            ),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
        ) {
            continue;
        }
        let mut equality = CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left: Box::new(CheckedScalarExpression::StructuralParameterByteLength {
                parameter_position,
                path: path.clone(),
            }),
            right: Box::new(integer_literal(
                u64::try_from(bytes.len()).ok()?,
                PrimitiveType::U64,
            )?),
        };
        for (index, byte) in bytes.iter().enumerate() {
            let read = CheckedScalarExpression::StructuralParameterIndexedRead {
                parameter_position,
                path: path.clone(),
                index: Box::new(integer_literal(
                    u64::try_from(index).ok()?,
                    PrimitiveType::U64,
                )?),
                primitive_type: PrimitiveType::U8,
            };
            let comparison = CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(read),
                right: Box::new(integer_literal(u64::from(*byte), PrimitiveType::U8)?),
            };
            equality = CheckedBooleanExpression::And {
                left: Box::new(equality),
                right: Box::new(comparison),
            };
        }
        return Some(equality);
    }
    None
}

fn integer_literal(value: u64, primitive_type: PrimitiveType) -> Option<CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: ArithmeticDomain::Exact,
    });
    Some(CheckedScalarExpression::IntegerLiteral { literal })
}

/// Comparison normalization may swap completed values for `>` and `>=`; it
/// does not change the source operand evaluation order retained by the caller.
pub(crate) fn construct_integer_comparison(
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

pub(crate) fn lower_boolean_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if let Some(value) = lower_closed_integer_literal_guard(program, operators, expression) {
        return Some(value);
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
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
                    authored_parameters,
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
                    authored_parameters,
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
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )
}
