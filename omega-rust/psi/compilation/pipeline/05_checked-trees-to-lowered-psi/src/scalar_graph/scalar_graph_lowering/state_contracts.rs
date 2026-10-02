//! Rejoin state requirements before making them block hypotheses.
//!
//! A tail-call fusion changes invocation into an ordinary CFG arrival. The
//! foreign entry's machine requirements therefore travel with that header,
//! in addition to its authored state clauses. Checking the source earlier is
//! not authority to omit either roster or substitute a same-typed predicate.

use crate::expression_preparation::source_custody;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedScalarStateGraph, CheckedTrees, ClosedScalarContractValue,
};

#[cfg(test)]
mod tests;

pub(super) fn replay(
    checked: &CheckedTrees,
    dispatch: symbols::SymbolHandle,
    retained: &CheckedScalarStateGraph,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    let (owner, source) = source_custody::authored_state(checked, retained.state)?;
    if checked
        .state_contracts(source)
        .iter()
        .any(|contract| contract.binding.is_some())
    {
        return unsupported("scalar state requirement cannot erase an evidence binding");
    }
    let predicates =
        validation::structural_state_contract_scalar_predicates(&checked.typed, source)
            .or_else(|| {
                validation::scalar_state_contracts_are_qualifications(&checked.typed, source)
                    .then(Vec::new)
            })
            .ok_or(LoweringError::Unsupported(
                "scalar state contract has no retained predicate reading",
            ))?;
    if predicates.len() != retained.requires.len() {
        return unsupported("scalar state requirement roster disagrees with source");
    }
    for (source_row, clause) in predicates.into_iter().zip(&retained.requires) {
        let Some(ClosedScalarContractValue::Predicate(predicate)) = clause else {
            return unsupported("scalar state requirement lost its predicate");
        };
        match source_row {
            validation::StateScalarContractRow::Expression(expression) => {
                validate_expression(checked, source.symbol, expression, predicate)?;
            }
            validation::StateScalarContractRow::Interval {
                parameter,
                minimum,
                maximum,
            } => {
                let parameters = checked.state_parameters(source);
                let authored = parameters
                    .iter()
                    .position(|source| source.symbol == parameter)
                    .ok_or(LoweringError::Unsupported(
                        "state interval lost its source parameter",
                    ))?;
                let primitive_type = checked
                    .primitive_type_reference(parameters[authored].type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "state interval lost its scalar carrier",
                    ))?;
                let position = parameters[..authored]
                    .iter()
                    .filter(|parameter| {
                        !parameter.relevance.is_erased()
                            && checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .count();
                let subject = CheckedScalarExpression::Parameter {
                    position,
                    primitive_type,
                };
                let minimum = validation::land_integer_value(&minimum, primitive_type).ok_or(
                    LoweringError::Unsupported("state interval minimum cannot inhabit its carrier"),
                )?;
                let maximum = validation::land_integer_value(&maximum, primitive_type).ok_or(
                    LoweringError::Unsupported("state interval maximum cannot inhabit its carrier"),
                )?;
                let CheckedBooleanExpression::And { left, right } = predicate else {
                    return unsupported("scalar state interval lost its bound conjunction");
                };
                let (
                    CheckedBooleanExpression::IntegerComparison { left: lower, .. },
                    CheckedBooleanExpression::IntegerComparison { right: upper, .. },
                ) = (left.as_ref(), right.as_ref())
                else {
                    return unsupported("scalar state interval lost its comparisons");
                };
                let (
                    CheckedScalarExpression::IntegerLiteral { literal: lower },
                    CheckedScalarExpression::IntegerLiteral { literal: upper },
                ) = (lower.as_ref(), upper.as_ref())
                else {
                    return unsupported("scalar state interval lost its literal bounds");
                };
                if lower.landing() != minimum.landing() || upper.landing() != maximum.landing() {
                    return unsupported("scalar state interval changed its literal carrier");
                }
                let expected = CheckedBooleanExpression::And {
                    left: Box::new(CheckedBooleanExpression::IntegerComparison {
                        kind: CheckedIntegerComparisonKind::LessOrEqual,
                        left: Box::new(CheckedScalarExpression::IntegerLiteral {
                            literal: minimum,
                        }),
                        right: Box::new(subject.clone()),
                    }),
                    right: Box::new(CheckedBooleanExpression::IntegerComparison {
                        kind: CheckedIntegerComparisonKind::LessOrEqual,
                        left: Box::new(subject),
                        right: Box::new(CheckedScalarExpression::IntegerLiteral {
                            literal: maximum,
                        }),
                    }),
                };
                if predicate != &expected {
                    return unsupported("scalar state interval predicate disagrees with source");
                }
            }
            validation::StateScalarContractRow::Finite { parameter } => {
                // Rederived from the declaration, never read from the plan.
                if validation::state_finite_predicate(&checked.typed, source, parameter).as_ref()
                    != Some(predicate)
                {
                    return unsupported("scalar state finite predicate disagrees with source");
                }
            }
        }
    }
    let mut requirements = retained.requires.clone();
    if owner.symbol != dispatch
        && checked
            .machine_states(owner)
            .first()
            .is_some_and(|entry| entry.symbol == source.symbol)
    {
        let plan = super::contract_lowering::closed_scalar_contract_plan(checked, owner.symbol)?;
        crate::unit::runtime_requirements::validate_graph_parameter_ranges(
            checked,
            owner.symbol,
            plan,
        )?;
        let mut clauses = plan.requires().iter();
        for contract in checked
            .machine_contracts(owner)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            if contract.binding.is_some() {
                return unsupported("fused entry requirement cannot erase an evidence binding");
            }
            let Some(Some(ClosedScalarContractValue::Predicate(predicate))) = clauses.next() else {
                return unsupported("fused entry requirement has no scalar predicate custody");
            };
            let facts =
                checked
                    .proof_facts
                    .span(contract.facts)
                    .ok_or(LoweringError::Unsupported(
                        "fused entry requirement has an invalid fact span",
                    ))?;
            validate_facts(checked, source.symbol, facts, predicate)?;
        }
        // Parameter-range tails were independently rejoined above. Floating
        // ranges need a block-scoped float range owner, not silent omission.
        if plan
            .requires()
            .iter()
            .any(|clause| !matches!(clause, Some(ClosedScalarContractValue::Predicate(_))))
        {
            return unsupported("fused entry requirement needs a non-predicate evidence owner");
        }
        requirements.extend_from_slice(plan.requires());
    }
    Ok(requirements)
}

fn validate_facts(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    facts: &[ProofFact],
    predicate: &CheckedBooleanExpression,
) -> Result<(), LoweringError> {
    match facts {
        [ProofFact::Expression(expression)] => {
            validate_expression(checked, state, *expression, predicate)
        }
        [prefix @ .., ProofFact::Expression(expression)] if !prefix.is_empty() => {
            let CheckedBooleanExpression::And { left, right } = predicate else {
                return unsupported("fused entry requirement lost its authored conjunction");
            };
            validate_facts(checked, state, prefix, left)?;
            validate_expression(checked, state, *expression, right)
        }
        _ => unsupported("fused entry requirement needs its source fact owner"),
    }
}

fn validate_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    expression: checked_trees::expression::ExpressionHandle,
    predicate: &CheckedBooleanExpression,
) -> Result<(), LoweringError> {
    let scalar = CheckedScalarExpression::Boolean(Box::new(predicate.clone()));
    source_custody::validate_entry_read_expression(checked, state, expression, &scalar)?;
    source_custody::value_correspondence::validate(
        checked,
        state,
        0,
        expression,
        PrimitiveType::Bool,
        &checked_trees::CheckedCallScalarArgument::Pure(scalar),
    )?;
    if !predicate_literal_landings(checked, expression, predicate, 0) {
        return unsupported("state requirement changed its integer literal landing");
    }
    Ok(())
}

/// Read and operator correspondence above owns expression meaning. This walk
/// additionally preserves literal landing metadata, which IntegerLiteral's
/// spelling equality deliberately omits. Closed contracts retain literal atoms
/// and exact arithmetic, not arbitrary folded body computations.
fn predicate_literal_landings(
    checked: &CheckedTrees,
    source: checked_trees::expression::ExpressionHandle,
    predicate: &CheckedBooleanExpression,
    depth: usize,
) -> bool {
    use checked_trees::expression::{BinaryOperator, ExpressionNode};
    if depth >= 64 {
        return false;
    }
    let node = checked.expression_table.expression(source);
    match predicate {
        CheckedBooleanExpression::Not(operand) => {
            let inner = match node {
                ExpressionNode::Unary(unary) => unary.operand,
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::NotEqual => {
                    source
                }
                _ => return false,
            };
            predicate_literal_landings(checked, inner, operand, depth + 1)
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right }
        | CheckedBooleanExpression::Equal { left, right } => {
            let ExpressionNode::Binary(binary) = node else {
                return false;
            };
            predicate_literal_landings(checked, binary.left, left, depth + 1)
                && predicate_literal_landings(checked, binary.right, right, depth + 1)
        }
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            let ExpressionNode::Binary(binary) = node else {
                return false;
            };
            let (left_source, right_source) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (binary.right, binary.left)
            } else {
                (binary.left, binary.right)
            };
            [(left_source, left), (right_source, right)]
                .into_iter()
                .all(|(source, retained)| {
                    source_custody::value_correspondence::validate_embedding_sources(
                        checked,
                        source,
                        retained,
                        |source, operand| {
                            scalar_literal_landings(checked, source, operand, depth + 1)
                        },
                    )
                })
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            let ExpressionNode::Binary(binary) = node else {
                return false;
            };
            let (left_source, right_source) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (binary.right, binary.left)
            } else {
                (binary.left, binary.right)
            };
            left.primitive_type().is_some()
                && left.primitive_type() == right.primitive_type()
                && scalar_literal_landings(checked, left_source, left, depth + 1)
                && scalar_literal_landings(checked, right_source, right, depth + 1)
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. } => true,
        _ => false,
    }
}

fn scalar_literal_landings(
    checked: &CheckedTrees,
    source: checked_trees::expression::ExpressionHandle,
    retained: &CheckedScalarExpression,
    depth: usize,
) -> bool {
    use checked_trees::expression::ExpressionNode;
    if depth >= 64 {
        return false;
    }
    match retained {
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let ExpressionNode::Integer(authored) = checked.expression_table.expression(source)
            else {
                // The closed subject reader also folds a raw fixed array's
                // type-owned extent. Read/value custody above has already
                // rejoined that member to its exact array and length.
                return matches!(
                    checked.expression_table.expression(source),
                    ExpressionNode::Member(_)
                ) && literal.landing()
                    == Some(numerics::literals::IntegerLanding {
                        landed_type: numerics::literals::LandedIntegerType::U64,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    });
            };
            if authored.landing().is_some() {
                return authored.landing() == literal.landing();
            }
            retained
                .primitive_type()
                .and_then(|primitive| {
                    validation::land_integer_value(&authored.value_bignum()?, primitive)
                })
                .is_some_and(|expected| expected.landing() == literal.landing())
        }
        CheckedScalarExpression::IntegerBinary {
            left,
            right,
            primitive_type,
            ..
        } => {
            let ExpressionNode::Binary(binary) = checked.expression_table.expression(source) else {
                return false;
            };
            left.primitive_type() == Some(*primitive_type)
                && right.primitive_type() == Some(*primitive_type)
                && scalar_literal_landings(checked, binary.left, left, depth + 1)
                && scalar_literal_landings(checked, binary.right, right, depth + 1)
        }
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. } => true,
        _ => false,
    }
}
