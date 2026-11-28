//! A single comparison can report a mismatch, commit the proposal, or match
//! without committing. Each case constrains both the payload and the resident.

use super::storage::{Location, Storage, Stored};
use crate::synchronization::progress::ScalarFact;
use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicCompareExchangeOutcomeCase as Outcome,
    AtomicCompareExchangeOutcomePayload as Payload,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::{
    StructuralFieldType, StructuralOperationResult, StructuralPathSegment as Segment,
    StructuralTypeDeclaration,
    types::{atomic_once_outcome_cases, atomic_once_result_is_plain},
};

fn include(alternatives: &mut Option<Stored>, value: Stored) {
    match alternatives {
        Some(values) => values.join(&value),
        None => *alternatives = Some(value),
    }
}

/// Partition the possible residents by comparison. A repeated entry input
/// equals itself. Independent entry inputs can still denote the same word.
fn compare(
    prior: &Stored,
    ty: ScalarType,
    expected: Option<ScalarFact>,
) -> (Option<Stored>, Option<Stored>) {
    let mut matched = None;
    let mut mismatched = None;
    if prior
        .visit_scalars(ty, |observed| {
            let equal = match expected {
                Some(expected) if expected == observed => Some(true),
                Some(ScalarFact::Literal(_)) if matches!(observed, ScalarFact::Literal(_)) => {
                    Some(false)
                }
                _ => None,
            };
            if equal != Some(false) {
                // On equality the known literal, if present, is more precise
                // than an entry parameter with the same observed value.
                let value = match expected {
                    Some(literal @ ScalarFact::Literal(_)) => literal,
                    _ => observed,
                };
                include(&mut matched, Stored::Scalar(value));
            }
            if equal != Some(true) {
                include(&mut mismatched, Stored::Scalar(observed));
            }
        })
        .is_none()
    {
        return (Some(expected.into()), Some(prior.clone()));
    }
    if prior.includes_pending_provider() {
        include(&mut matched, expected.into());
        include(&mut mismatched, Stored::PENDING_PROVIDER);
    }
    (matched, mismatched)
}

pub(super) fn evaluate(
    storage: &mut Storage,
    target: Location,
    declaration: &StructuralTypeDeclaration,
    result: &StructuralOperationResult,
    custody: AtomicCompareExchangeOnceResultCustody,
    expected: Option<ScalarFact>,
    proposal: Option<ScalarFact>,
) -> Option<Location> {
    if !atomic_once_result_is_plain(result, custody) {
        return None;
    }
    let field = declaration.shape.cases().first()?.fields.first()?;
    let StructuralFieldType::Scalar(ty) = field.field_type else {
        return None;
    };
    let cases = atomic_once_outcome_cases(declaration, ty, custody)?;
    let (matched, mismatched) = compare(&storage.get(&target), ty, expected);
    let proposal = Stored::from(proposal);
    let mut result = None;
    let mut next = None;
    let mut effects = Vec::new();
    for (case, schema) in cases.iter().zip(custody.outcome_identity.cases()) {
        let prior = match schema.case {
            Outcome::Mismatched => &mismatched,
            Outcome::Exchanged | Outcome::Uncommitted => &matched,
        };
        let Some(prior) = prior else { continue };
        let resident = if schema.case == Outcome::Exchanged {
            &proposal
        } else {
            prior
        };
        let mut variant = Stored::variant(case.id);
        if let Some(payload) = schema.payload {
            let [field] = case.fields.as_slice() else {
                return None;
            };
            let value = match payload {
                Payload::Proposed => &proposal,
                Payload::Observed | Payload::Displaced => prior,
            };
            *variant.project_mut(&[Segment::Case(case.id), Segment::Field(field.identity)]) =
                value.clone();
        }
        include(&mut result, variant);
        include(&mut next, resident.clone());
        effects.push((case.id, resident.clone()));
    }
    Some(storage.allocate_outcome(result?, target, next?, effects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_values::TerminalScalarValue as Scalar;

    #[test]
    fn a_selected_provider_can_supply_a_different_word_from_the_local_alternative() {
        let mut prior = Stored::ZERO;
        prior.join(&Stored::PENDING_PROVIDER);
        for value in [false, true] {
            let expected = ScalarFact::Literal(Scalar::Boolean(value));
            let (matched, mismatched) = compare(&prior, ScalarType::Boolean, Some(expected));
            assert_eq!(matched.unwrap().scalar(ScalarType::Boolean), Some(expected));
            let mismatched = mismatched.unwrap();
            assert!(mismatched.includes_pending_provider());
            let mut local = Vec::new();
            mismatched.visit_scalars(ScalarType::Boolean, |word| local.push(word));
            if value {
                assert_eq!(local, [ScalarFact::Literal(Scalar::Boolean(false))]);
            } else {
                assert!(local.is_empty());
            }
        }
    }
}
