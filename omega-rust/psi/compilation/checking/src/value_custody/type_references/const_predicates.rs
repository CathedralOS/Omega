//! Closed structural indices owe their data predicates at use, independently
//! of their canonical encoding. Carrier eligibility answers whether identity is
//! structural; it cannot decide membership for every value of that carrier.
//! Bind decoded fields by declaration symbol and rejoin selected operators
//! before evaluating a predicate. Unknown expressions supply no evidence.

use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use typed_trees::data::{DataDefinition, DataField};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    facts: arena::HandleSpan<ProofFact>,
    fields: &[&DataField],
    values: &[(String, DecodedCanonicalConstValue)],
) -> Result<(), String> {
    for fact in program.proof_facts.span_or_empty(facts) {
        let verdict = fact
            .as_expression()
            .ok_or("membership and proposition predicates need their own index-use proof")
            .and_then(|expression| evaluate(program, definition, fields, values, *expression, 0))
            .and_then(|(value, _)| match value {
                Scalar::Boolean(true) => Ok(()),
                Scalar::Boolean(false) => Err("the required predicate is false"),
                Scalar::Integer(_) => Err("the required predicate is not Boolean"),
            });
        if let Err(reason) = verdict {
            return Err(format!(
                "canonical value of `{}` does not establish its required default-domain predicate at index use: {reason}",
                definition.name
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Scalar {
    Integer(i128),
    Boolean(bool),
}

fn scalar(value: &DecodedCanonicalConstValue) -> Option<Scalar> {
    match value {
        DecodedCanonicalConstValue::Integer { value, .. } => Some(Scalar::Integer(*value)),
        DecodedCanonicalConstValue::Boolean(value) => Some(Scalar::Boolean(*value)),
        _ => None,
    }
}

fn evaluate(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    fields: &[&DataField],
    values: &[(String, DecodedCanonicalConstValue)],
    expression: ExpressionHandle,
    depth: usize,
) -> Result<(Scalar, Option<TypeReferenceHandle>), &'static str> {
    if depth > 128 {
        return Err("closed scalar predicate exceeds the evaluation depth bound");
    }
    if !program.expression_table.expression_is_valid(expression) {
        return Err("closed scalar predicate has an invalid expression handle");
    }
    let operand = |expression| evaluate(program, definition, fields, values, expression, depth + 1);
    let boolean = |value| {
        let carrier = crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
            program,
            symbols::BuiltinTypeAtom::Bool,
        )
        .ok_or("closed scalar predicate has no resolved Boolean carrier")?;
        Ok((Scalar::Boolean(value), Some(carrier)))
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Ok((
            Scalar::Integer(
                value
                    .text()
                    .parse()
                    .map_err(|_| "integer predicate literal exceeds the evaluation carrier")?,
            ),
            None,
        )),
        ExpressionNode::Boolean(value) => boolean(*value),
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid()
                || program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
            {
                return Err("predicate value is not one resolved lexical declaration");
            }
            if let Some(field) = fields.iter().find(|field| field.symbol == path.symbol) {
                let (_, value) = values
                    .iter()
                    .find(|(name, _)| name == field.name.as_str())
                    .ok_or("predicate field has no completed value")?;
                return Ok((
                    scalar(value)
                        .ok_or("predicate field is outside the closed integer/Boolean fragment")?,
                    Some(field.type_reference),
                ));
            }
            let declaration = program
                .const_declaration_by_symbol(path.symbol)
                .ok_or("predicate name is neither its field nor an evaluated constant")?;
            let encoding = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
                .ok_or("predicate constant has no completed canonical value")?;
            let value = CanonicalConstValue::new("", encoding, "")
                .decode_encoding()
                .ok_or("predicate constant has a malformed canonical encoding")?;
            Ok((
                scalar(&value)
                    .ok_or("predicate constant is outside the closed integer/Boolean fragment")?,
                Some(declaration.declared_type),
            ))
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let (Scalar::Boolean(value), _) = operand(unary.operand)? else {
                return Err("logical negation requires a Boolean predicate value");
            };
            boolean(!value)
        }
        ExpressionNode::Binary(binary) => {
            let (left, left_type) = operand(binary.left)?;
            let (right, right_type) = operand(binary.right)?;
            if let Some(spelling) =
                crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                    binary.operator,
                )
                && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    definition.symbol,
                    expression,
                    spelling,
                    &[left_type, right_type],
                )
            {
                return Err("predicate operator does not select builtin scalar meaning");
            }
            let value = match (left, right) {
                (Scalar::Integer(left), Scalar::Integer(right)) => match binary.operator {
                    BinaryOperator::Equal => left == right,
                    BinaryOperator::NotEqual => left != right,
                    BinaryOperator::Less => left < right,
                    BinaryOperator::LessOrEqual => left <= right,
                    BinaryOperator::Greater => left > right,
                    BinaryOperator::GreaterOrEqual => left >= right,
                    _ => {
                        return Err(
                            "integer predicate operation is outside the closed comparison fragment",
                        );
                    }
                },
                (Scalar::Boolean(left), Scalar::Boolean(right)) => match binary.operator {
                    BinaryOperator::Equal => left == right,
                    BinaryOperator::NotEqual => left != right,
                    BinaryOperator::And => left && right,
                    BinaryOperator::Or => left || right,
                    _ => {
                        return Err(
                            "Boolean predicate operation is outside the closed logical fragment",
                        );
                    }
                },
                _ => return Err("predicate operands do not share integer or Boolean meaning"),
            };
            boolean(value)
        }
        _ => Err("predicate expression is outside the closed scalar fragment"),
    }
}
