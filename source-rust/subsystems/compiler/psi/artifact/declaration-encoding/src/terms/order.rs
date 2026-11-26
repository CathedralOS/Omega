//! Check the order of clauses and operands in encoded propositions.
use crate::CodecError;
use crate::terms::proposition::canonical_proposition_order_key;
use crate::terms::scalar::canonical_scalar_term_order_key;
use semantic_vocabulary::Proposition;

pub fn validate_canonical_proposition(proposition: &Proposition) -> Result<(), CodecError> {
    enum Step<'a> {
        Validate(&'a Proposition),
        StrictlyIncreasing(&'a [Proposition], &'static str),
    }
    let mut pending = vec![Step::Validate(proposition)];
    while let Some(step) = pending.pop() {
        match step {
            Step::Validate(proposition) => match proposition {
                Proposition::FloatIntegerConversion(_)
                | Proposition::IeeeFloatIntegerExact { .. }
                | Proposition::IntegerIeeeFloatExact { .. } => {
                    proposition
                        .validate()
                        .map_err(CodecError::MalformedProposition)?;
                }
                Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                Proposition::Equal(left, right) => {
                    if canonical_scalar_term_order_key(left)?
                        > canonical_scalar_term_order_key(right)?
                    {
                        return Err(CodecError::NonCanonicalOrder("equality operands"));
                    }
                }
                Proposition::LessThan(_, _) | Proposition::LessOrEqual(_, _) => {}
                Proposition::IntegerMathEqual(left, right) => {
                    if left > right {
                        return Err(CodecError::NonCanonicalOrder(
                            "mathematical integer equality operands",
                        ));
                    }
                }
                Proposition::IntegerMathLessThan(_, _)
                | Proposition::IntegerMathLessOrEqual(_, _) => {}
                Proposition::IeeeFloatComparison {
                    kind, left, right, ..
                } => {
                    if kind.is_symmetric() && left > right {
                        return Err(CodecError::NonCanonicalOrder("IEEE equality operands"));
                    }
                }
                Proposition::ScalarIeeeFloatComparison {
                    kind, left, right, ..
                } => {
                    if kind.is_symmetric() && left > right {
                        return Err(CodecError::NonCanonicalOrder(
                            "scalar IEEE equality operands",
                        ));
                    }
                }
                Proposition::StructuralCaseMembership { .. } => {}
                Proposition::DeclarationIdentity(clause) => clause
                    .validate()
                    .map_err(CodecError::MalformedProposition)?,
                Proposition::IsMere(inner) => pending.push(Step::Validate(inner)),
                Proposition::Conjunction(conjuncts) => {
                    if conjuncts
                        .iter()
                        .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
                    {
                        return Err(CodecError::NestedConjunction);
                    }
                    pending.push(Step::StrictlyIncreasing(
                        conjuncts,
                        "conjunction propositions",
                    ));
                    for conjunct in conjuncts.iter().rev() {
                        pending.push(Step::Validate(conjunct));
                    }
                }
                Proposition::Disjunction(disjuncts) => {
                    if disjuncts
                        .iter()
                        .any(|disjunct| matches!(disjunct, Proposition::Disjunction(_)))
                    {
                        return Err(CodecError::NestedDisjunction);
                    }
                    pending.push(Step::StrictlyIncreasing(
                        disjuncts,
                        "disjunction propositions",
                    ));
                    for disjunct in disjuncts.iter().rev() {
                        pending.push(Step::Validate(disjunct));
                    }
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(Step::Validate(conclusion));
                    pending.push(Step::Validate(premise));
                }
                Proposition::ContentConservation(_) => {}
            },
            Step::StrictlyIncreasing(propositions, order_label) => {
                if !canonical_propositions_strictly_increase(propositions)? {
                    return Err(CodecError::NonCanonicalOrder(order_label));
                }
            }
        }
    }
    Ok(())
}

pub fn canonical_propositions_strictly_increase(
    propositions: &[Proposition],
) -> Result<bool, CodecError> {
    let mut previous = None;
    for proposition in propositions {
        let bytes = canonical_proposition_order_key(proposition)?;
        if previous.as_ref().is_some_and(|previous| previous >= &bytes) {
            return Ok(false);
        }
        previous = Some(bytes);
    }
    Ok(true)
}
