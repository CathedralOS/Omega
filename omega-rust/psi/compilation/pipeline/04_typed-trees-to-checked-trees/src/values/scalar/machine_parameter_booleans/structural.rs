//! Entry-parameter predicates that read structural parameters: Boolean
//! fields, case membership, structural equality, and numeric comparisons
//! whose operands read structural fields. Structural
//! places keep authored parameter positions; scalar parameters use the dense
//! scalar namespace.

use super::structural_equality;
use super::structural_paths::{path_primitive_type, path_type_reference};
use crate::values::scalar::boolean_lowering::construct_integer_comparison;
use crate::values::scalar::expression_facts::{operator_is_builtin, scalar_expression_type};
use crate::values::scalar::scalar_lowering::lower_scalar_operands;
use crate::values::scalar::structural_fields::{
    structural_data, structural_parameter_case_path, structural_parameter_field_path,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedStructuralParameterField,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Lower `expression` in the entry parameters' namespace, or `None` when no
/// structural form describes it.
pub(crate) fn lower(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
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
            observation: expression,
            parameter_position,
            path,
        });
    }

    if let Some(binary) = program.expression_table.binary_node(expression)
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
                position: crate::values::scalar::scalar_position_count(
                    program,
                    &parameters[..source_position],
                ),
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
            if binary.operator.is_comparison() && operator_is_builtin(operators, expression) =>
        {
            let numeric_comparison = (|| {
                // Structural reads and scalar operations share operand landing
                // and arithmetic policy. Keep the dense scalar roster separate
                // from authored structural positions; entry admission and
                // independent source replay still validate the resulting reads.
                let (scalar_parameters, parameter_types) =
                    crate::values::scalar::scalar_parameter_roster(program, parameters)?;
                let ((left, _), (right, _)) = lower_scalar_operands(
                    program,
                    operators,
                    binary,
                    &scalar_parameters,
                    parameters,
                    &parameter_types,
                    &[],
                    &[],
                )?;
                if scalar_expression_type(&left) == Some(PrimitiveType::Addr)
                    || scalar_expression_type(&right) == Some(PrimitiveType::Addr)
                {
                    return None;
                }
                if matches!(
                    scalar_expression_type(&left),
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                ) {
                    let references = [binary.left, binary.right].map(|operand| {
                        validation::parameter_expression_result_type_reference(
                            program,
                            machine.symbol,
                            parameters,
                            operand,
                        )
                    });
                    return crate::values::scalar::result_contract::lower_float_contract_comparison(
                        program,
                        machine.symbol,
                        expression,
                        [Some((left, references[0]?)), Some((right, references[1]?))],
                    );
                }
                construct_integer_comparison(binary.operator, left, right)
            })();
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || numeric_comparison.is_some()
            {
                return numeric_comparison;
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

fn lower_structural_case_membership(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    let state = program.machine_states(machine).first()?;
    if !validation::has_builtin_binary_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let membership = validation::has_exact_case_membership_meaning(
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
                    && program
                        .data_members(data)
                        .iter()
                        .any(|member| matches!(member, typed_trees::data::DataMember::Field(_))))
            {
                return None;
            }
            program.data_members(data).iter().find_map(|member| {
                let variant = member.as_variant()?;
                (variant.symbol == case_symbol
                    && (membership || program.data_payload_fields(variant).is_empty()))
                .then(|| {
                    (
                        data.symbol,
                        checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        ),
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
    let parameter_position =
        structural_parameter_case_path(program, parameters, subject_expression, &mut path)?;
    let subject_type = path_type_reference(program, parameters, parameter_position, &path)?;
    (structural_data(program, subject_type)?.symbol == data_symbol).then_some(
        CheckedBooleanExpression::StructuralCaseMembership {
            subject: CheckedStructuralParameterField {
                parameter_position,
                path,
            }
            .into(),
            case,
        },
    )
}
