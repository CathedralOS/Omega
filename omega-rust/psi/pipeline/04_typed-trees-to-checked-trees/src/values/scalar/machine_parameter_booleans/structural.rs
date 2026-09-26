//! Entry-parameter predicates that read structural parameters: Boolean
//! fields, case membership, structural equality, IEEE field equality, and
//! integer comparisons whose operands read structural fields. Structural
//! places keep authored parameter positions; scalar parameters use the dense
//! scalar namespace.

use super::structural_equality;
use super::structural_paths::{path_primitive_type, path_type_reference};
use crate::checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedOperatorFacts, CheckedScalarExpression,
    CheckedStructuralParameterField,
};
use crate::values::scalar::expression_facts::{
    checked_integer_binary_kind, combine_arithmetic_domains, is_integer, operator_is_builtin,
    parameter_position, scalar_expression_type,
};
use crate::values::scalar::scalar_lowering::landed_for_primitive;
use crate::values::scalar::structural_fields::{structural_data, structural_parameter_field_path};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLanding;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType;

/// Lower `expression` in the entry parameters' namespace, or `None` when no
/// structural form describes it.
pub(super) fn lower(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if let Some(membership) =
        lower_structural_case_membership(program, machine, parameters, expression)
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
            structural_equality::lower(program, parameters, binary.left, binary.right)
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
            if parameter.relevance.is_erased() {
                return None;
            }
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
                        crate::values::scalar::occupies_scalar_position(program, parameter)
                    })
                    .count(),
            })
        }
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && operator_is_builtin(operators, expression) =>
        {
            Some(CheckedBooleanExpression::Not(Box::new(lower(
                program,
                machine,
                operators,
                parameters,
                unary.operand,
            )?)))
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
                let contextual_literal =
                    |expression: ExpressionHandle,
                     primitive_type: PrimitiveType|
                     -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
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
                left: Box::new(lower(program, machine, operators, parameters, binary.left)?),
                right: Box::new(lower(
                    program,
                    machine,
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
            let left = Box::new(lower(program, machine, operators, parameters, binary.left)?);
            let right = Box::new(lower(
                program,
                machine,
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
            // An erased binding has no scalar position to read.
            if parameter.relevance.is_erased() {
                return None;
            }
            let primitive_type = program.primitive_type_reference(parameter.type_reference)?;
            if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                return None;
            }
            // Scalar values use the dense primitive namespace, while
            // structural member roots above retain authored positions.
            let position = parameters[..source_position]
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
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
            let (operand, domain) =
                lower_structural_integer_expression(program, operators, parameters, unary.operand)?;
            let primitive_type = scalar_expression_type(&operand)?;
            (is_integer(primitive_type) && primitive_type != PrimitiveType::Addr).then_some((
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand: Box::new(operand),
                },
                domain,
            ))
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
            ) && operator_is_builtin(operators, expression) =>
        {
            let (left, left_domain) =
                lower_structural_integer_expression(program, operators, parameters, binary.left)?;
            let (right, right_domain) =
                lower_structural_integer_expression(program, operators, parameters, binary.right)?;
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
            let (left, left_domain) =
                lower_structural_integer_expression(program, operators, parameters, binary.left)?;
            let (right, right_domain) =
                lower_structural_integer_expression(program, operators, parameters, binary.right)?;
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
            let (left, left_domain) =
                lower_structural_integer_expression(program, operators, parameters, binary.left)?;
            let (right, _) =
                lower_structural_integer_expression(program, operators, parameters, binary.right)?;
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
    let primitive_type = path_primitive_type(program, parameters, parameter_position, &path)?;
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
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    let state = program.machine_states(machine).first()?;
    if !crate::validation::has_builtin_binary_expression_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return None;
    }
    let membership = crate::validation::has_exact_case_membership_meaning(
        program,
        machine,
        Some(state),
        expression,
        binary,
    );
    let classifier = |candidate: ExpressionHandle| {
        let (case_symbol, literal_owner) = match program.expression_table.expression(candidate) {
            ExpressionNode::Name(name) => (name.symbol, None),
            ExpressionNode::StructLiteral(literal)
                if program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
            {
                (literal.case_symbol?, Some(literal.type_symbol))
            }
            _ => return None,
        };
        program.data_definitions().iter().find_map(|data| {
            if literal_owner.is_some_and(|owner| owner != data.symbol)
                || (!membership
                    && program.data_members(data).iter().any(|member| {
                        matches!(
                        member,
                        symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(
                            _
                        )
                    )
                    }))
            {
                return None;
            }
            program.data_members(data).iter().find_map(|member| {
                let symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Variant(
                    variant,
                ) = member
                else {
                    return None;
                };
                (variant.symbol == case_symbol
                    && (membership || program.data_payload_fields(variant).is_empty()))
                .then(|| (data.symbol, variant.path_identity()))
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
    let parameter_position =
        structural_parameter_field_path(program, parameters, subject_expression, &mut path)?;
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
