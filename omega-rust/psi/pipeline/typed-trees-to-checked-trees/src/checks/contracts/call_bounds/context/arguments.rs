//! Exact integer actuals admitted to mathematical call-requirement substitution.
//!
//! This is meaning custody, not an overflow proof. Normal source formation and
//! Terminal operation obligations still independently discharge Exact arithmetic.

use super::*;
use numerics::arithmetic::ArithmeticDomain;

pub(super) fn primitive_type(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    meaning(program, facts, owner, parameters, expression).map(|meaning| meaning.primitive)
}

struct Meaning {
    primitive: PrimitiveType,
    domain: ArithmeticDomain,
    type_reference: Option<TypeReferenceHandle>,
}

fn meaning(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<Meaning> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let parameter = direct_parameter(program, parameters, expression)?;
            Some(Meaning {
                primitive: fixed_parameter_type(program, parameter)?,
                domain: program.arithmetic_domain_for_type_reference(parameter.type_reference),
                type_reference: Some(parameter.type_reference),
            })
        }
        ExpressionNode::Integer(literal) => {
            let primitive = crate::values::scalar_expression_type(
                &checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal: literal.clone(),
                },
            )?;
            fixed_integer(primitive).then_some(Meaning {
                primitive,
                domain: literal.landing()?.domain,
                type_reference: None,
            })
        }
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                _ => return None,
            };
            let left = meaning(program, facts, owner, parameters, binary.left)?;
            let right = meaning(program, facts, owner, parameters, binary.right)?;
            if left.primitive != right.primitive
                || left.domain != ArithmeticDomain::Exact
                || right.domain != ArithmeticDomain::Exact
                // A missing checked row is not builtin authority: the typed
                // query below independently joins declared/trait meanings and
                // the retained authored selection. Existing checked rows veto
                // that reading when they selected anything non-builtin.
                // A compound operand has a plain builtin result, not a retained
                // source parameter's constrained/custom type reference.
                || !crate::checks::contracts::prover::has_builtin_operators(
                    program,
                    &facts.operators,
                    expression,
                )
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    &[left.type_reference, right.type_reference],
                )
            {
                return None;
            }
            Some(Meaning {
                primitive: left.primitive,
                domain: ArithmeticDomain::Exact,
                type_reference: None,
            })
        }
        _ => None,
    }
}
