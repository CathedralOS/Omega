//! Rejoin state requirements before making them block hypotheses.
//!
//! A tail-call fusion changes invocation into an ordinary CFG arrival. The
//! foreign entry's machine requirements therefore travel with that header,
//! in addition to its authored state clauses. Checking the source earlier is
//! not authority to omit either roster or substitute a same-typed predicate.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedScalarStateGraph, CheckedTrees, ClosedScalarContractValue,
};

pub(super) fn replay(
    checked: &CheckedTrees,
    dispatch: symbols::SymbolHandle,
    retained: &CheckedScalarStateGraph,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    let (owner, source) = crate::declarations::state_declaration(checked, retained.state)?;
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
            validation::StateScalarContractRow::Expression(_) => {}
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
                    CheckedScalarExpression::IntegerLiteral { .. },
                    CheckedScalarExpression::IntegerLiteral { .. },
                ) = (lower.as_ref(), upper.as_ref())
                else {
                    return unsupported("scalar state interval lost its literal bounds");
                };
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
        [ProofFact::Expression(_)] => Ok(()),
        [prefix @ .., ProofFact::Expression(expression)] if !prefix.is_empty() => {
            let CheckedBooleanExpression::And { left, right: _ } = predicate else {
                return unsupported("fused entry requirement lost its authored conjunction");
            };
            validate_facts(checked, state, prefix, left)?;
            Ok(())
        }
        _ => unsupported("fused entry requirement needs its source fact owner"),
    }
}
