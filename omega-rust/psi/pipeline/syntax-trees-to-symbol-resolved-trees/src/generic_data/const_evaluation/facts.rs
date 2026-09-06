//! Constant evaluation: facts.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::generic_data) enum ConstFactValue {
    Integer(i128),
    Boolean(bool),
}

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
pub(in crate::generic_data) fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    self_value: Option<i128>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(ConstFactValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            Ok(parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
                .copied()
                .map(ConstFactValue::Integer))
        }
        ExpressionNode::SelfValue => Ok(self_value.map(ConstFactValue::Integer)),
        ExpressionNode::Binary(binary) => {
            validate_anonymous_remainder(syntax, binary)?;
            let Some(left) = evaluate_const_fact_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_fact_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => Ok(None),
    }
}

/// Discharge `N in Domain` when `N` is a concrete const parameter and the
/// domain is defined by evaluable boolean facts over `self`. Machine-call facts
/// stay on the concrete record for typed build-time evaluation.
pub(in crate::generic_data) fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &syntax_trees::item::ProofMembershipFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    parameter_type_names: &HashMap<String, String>,
) -> Result<Option<bool>, String> {
    let ExpressionNode::Name(value_path) = syntax.expressions.expression(membership.value) else {
        return Ok(None);
    };
    let [parameter_name] = syntax.expressions.identifier_path_members(*value_path) else {
        return Ok(None);
    };
    let Some(value) = parameter_values.get(parameter_name.as_str()).copied() else {
        return Ok(None);
    };
    let Some(parameter_type) = parameter_type_names.get(parameter_name.as_str()) else {
        return Ok(None);
    };
    let domain_path = syntax
        .items
        .identifier_path_members(membership.domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let domain_name = if domain_path.contains("::") {
        domain_path
    } else {
        format!("{parameter_type}::{domain_path}")
    };
    evaluate_named_const_domain(
        syntax,
        &domain_name,
        parameter_type,
        value,
        const_values,
        &mut Vec::new(),
    )
}

pub(in crate::generic_data) fn evaluate_named_const_domain(
    syntax: &SyntaxTrees,
    domain_name: &str,
    carrier: &str,
    value: i128,
    const_values: &HashMap<String, i128>,
    visiting: &mut Vec<String>,
) -> Result<Option<bool>, String> {
    if visiting.iter().any(|name| name == domain_name) {
        return Ok(None);
    }
    let Some(domain) = syntax.root_items().find_map(|item| {
        let Item::Domain(domain) = item else {
            return None;
        };
        (domain.name.as_str() == domain_name).then_some(domain)
    }) else {
        return Ok(None);
    };
    let TypeReferenceNode::Named(domain_target) =
        syntax.type_references.type_reference(domain.target_type)
    else {
        return Ok(None);
    };
    if domain_target.as_str() != carrier {
        return Err(format!(
            "domain `{domain_name}` has carrier `{}`, but the const value has carrier `{carrier}`",
            domain_target.as_str(),
        ));
    }
    visiting.push(domain_name.to_owned());
    for fact in syntax.items.proof_facts(domain.facts) {
        let holds = match fact {
            ProofFact::Expression(expression) => evaluate_const_domain_expression(
                syntax,
                *expression,
                const_values,
                value,
                carrier,
                visiting,
            )?,
            ProofFact::Membership(membership) => {
                let Some(ConstFactValue::Integer(nested_value)) = evaluate_const_fact_expression(
                    syntax,
                    membership.value,
                    const_values,
                    &HashMap::new(),
                    Some(value),
                )?
                else {
                    visiting.pop();
                    return Ok(None);
                };
                let path = syntax
                    .items
                    .identifier_path_members(membership.domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                let nested_domain = if path.contains("::") {
                    path
                } else {
                    format!("{carrier}::{path}")
                };
                evaluate_named_const_domain(
                    syntax,
                    &nested_domain,
                    carrier,
                    nested_value,
                    const_values,
                    visiting,
                )?
                .map(ConstFactValue::Boolean)
            }
        };
        let Some(ConstFactValue::Boolean(holds)) = holds else {
            visiting.pop();
            return Ok(None);
        };
        if !holds {
            visiting.pop();
            return Ok(Some(false));
        }
    }
    visiting.pop();
    Ok(Some(true))
}

pub(in crate::generic_data) fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    self_value: i128,
    carrier: &str,
    visiting: &mut Vec<String>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Membership(membership) => {
            let Some(ConstFactValue::Integer(value)) = evaluate_const_fact_expression(
                syntax,
                membership.value,
                const_values,
                &HashMap::new(),
                Some(self_value),
            )?
            else {
                return Ok(None);
            };
            let path = syntax
                .expressions
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let domain_name = if path.contains("::") {
                path
            } else {
                format!("{carrier}::{path}")
            };
            evaluate_named_const_domain(
                syntax,
                &domain_name,
                carrier,
                value,
                const_values,
                visiting,
            )
            .map(|result| result.map(ConstFactValue::Boolean))
        }
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_domain_expression(
                syntax,
                binary.right,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => evaluate_const_fact_expression(
            syntax,
            expression,
            const_values,
            &HashMap::new(),
            Some(self_value),
        ),
    }
}

pub(in crate::generic_data) fn evaluate_const_fact_binary(
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstFactValue::Integer(left), ConstFactValue::Integer(right)) => match operator {
            Add => checked_fact_integer(left.checked_add(right), "addition"),
            Subtract => checked_fact_integer(left.checked_sub(right), "subtraction"),
            Multiply => checked_fact_integer(left.checked_mul(right), "multiplication"),
            Divide => left
                .checked_div(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "division by zero is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "remainder by zero is invalid".to_string()),
            ShiftLeft if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shl(amount))
                .and_then(const_integer_in_envelope)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
            ShiftRight if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
            BitwiseAnd if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left & right)),
            BitwiseOr if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left | right)),
            BitwiseXor if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left ^ right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            Greater => Ok(ConstFactValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
            Less => Ok(ConstFactValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
            And | Or => Err("logical operators require boolean operands".to_string()),
            ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Err(
                "signed shifts and bitwise operators require declared-width semantics".to_string(),
            ),
        },
        (ConstFactValue::Boolean(left), ConstFactValue::Boolean(right)) => match operator {
            And => Ok(ConstFactValue::Boolean(left && right)),
            Or => Ok(ConstFactValue::Boolean(left || right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            _ => Err("arithmetic and ordering operators require integer operands".to_string()),
        },
        _ => Err("const fact operands have incompatible types".to_string()),
    }
}
