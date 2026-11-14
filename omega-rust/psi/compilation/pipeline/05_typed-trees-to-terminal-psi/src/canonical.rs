/*
Terminal Psi's canonical forms for the propositions this crate builds, one
route each, shared by authored clauses, published crash routes, crash site
guards and crash continuations substituted with a call's actuals. The codec
refuses anything else (`terminal-codec` `semantic_module.rs`:
`validate_canonical_proposition` and `crash_routes_are_canonical`). Order
comes from the codec's own keys or the vocabulary's derived order, never from
a printed form.
*/

use semantic_vocabulary::{CrashCause, MachineId, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

use crate::{Unserved, refuse};

/// What a repeated conjunct means where it arises.
#[derive(Clone, Copy)]
pub(crate) enum Repeated {
    /// An authored clause list: a repeat cannot be held in canonical order
    /// ("strictly increase"), so it is refused as authored.
    Refuse,
    /// A conjunction made by substitution or path collection: `P && P` is
    /// `P`, so a repeat is kept once.
    Merge,
}

/// `left == right` with its operands in canonical order.
pub(crate) fn equal(left: ScalarTerm, right: ScalarTerm, id: MachineId) -> Checked<Proposition> {
    let key = |term: &ScalarTerm| {
        terminal_codec::canonical_scalar_term_order_key(term).map_err(|_| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("unencodable term"),
            )
        })
    };
    if key(&left)? <= key(&right)? {
        Ok(Proposition::Equal(left, right))
    } else {
        Ok(Proposition::Equal(right, left))
    }
}

/// Sorts propositions into Terminal Psi's canonical order.
pub(crate) fn clauses(
    propositions: Vec<Proposition>,
    repeated: Repeated,
    id: MachineId,
) -> Checked<Vec<Proposition>> {
    associated_clauses(
        propositions
            .into_iter()
            .map(|proposition| (proposition, ()))
            .collect(),
        repeated,
        id,
    )
    .map(|rows| {
        rows.into_iter()
            .map(|(proposition, ())| proposition)
            .collect()
    })
}

/// Keep a clause's source description beside it while fixing canonical order.
/// Associated payloads never participate in proposition identity or ordering.
pub(crate) fn associated_clauses<T>(
    propositions: Vec<(Proposition, T)>,
    repeated: Repeated,
    id: MachineId,
) -> Checked<Vec<(Proposition, T)>> {
    let mut keyed = Vec::with_capacity(propositions.len());
    for (proposition, associated) in propositions {
        let key = terminal_codec::canonical_proposition_order_key(&proposition).map_err(|_| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("unencodable proposition"),
            )
        })?;
        keyed.push((key, proposition, associated));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    let repeats = keyed.windows(2).any(|pair| pair[0].0 == pair[1].0);
    match repeated {
        Repeated::Refuse if repeats => {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("duplicate clause"),
            ));
        }
        Repeated::Refuse | Repeated::Merge => {}
    }
    keyed.dedup_by(|later, earlier| later.0 == earlier.0);
    Ok(keyed
        .into_iter()
        .map(|(_, proposition, associated)| (proposition, associated))
        .collect())
}

/// A conjunction in canonical form: flattened, ordered, a repeat kept once,
/// and a single conjunct standing alone.
pub(crate) fn conjunction(conjuncts: Vec<Proposition>, id: MachineId) -> Checked<Proposition> {
    let mut flat = Vec::with_capacity(conjuncts.len());
    for conjunct in conjuncts {
        match conjunct {
            Proposition::Conjunction(inner) => flat.extend(inner),
            other => flat.push(other),
        }
    }
    let mut ordered = clauses(flat, Repeated::Merge, id)?;
    match ordered.len() {
        0 => Ok(Proposition::Truth),
        1 => Ok(ordered.remove(0)),
        _ => Ok(Proposition::Conjunction(ordered)),
    }
}

/// Crash routes in canonical form: one bucket per cause in increasing cause
/// order, alternatives strictly increasing, and `Truth` alone when any
/// alternative is `Truth` (it permits every other alternative).
pub(crate) fn routes(
    alternatives: impl IntoIterator<Item = (CrashCause, CrashRouteGuard)>,
) -> Vec<CrashRouteBucket> {
    let mut alternatives: Vec<_> = alternatives.into_iter().collect();
    alternatives.sort();
    alternatives.dedup();
    let mut buckets: Vec<CrashRouteBucket> = Vec::new();
    for (cause, guard) in alternatives {
        match buckets.last_mut() {
            Some(bucket) if bucket.cause == cause => bucket.alternatives.push(guard),
            Some(_) | None => buckets.push(CrashRouteBucket {
                cause,
                alternatives: vec![guard],
            }),
        }
    }
    for bucket in &mut buckets {
        if bucket.alternatives.contains(&CrashRouteGuard::Truth) {
            bucket.alternatives = vec![CrashRouteGuard::Truth];
        }
    }
    buckets
}

/// A callee's published routes at one call: every formal replaced by the
/// actual at its position. calls_and_outcomes.md: "Surviving crash routes are
/// derived from the pinned callee contract using exact actuals. ... Empty or
/// untranslated continuation rows cannot erase a crash." No route is dropped
/// here; disproving one is the checker's work.
pub(crate) fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
    id: MachineId,
) -> Checked<Vec<CrashRouteBucket>> {
    let mut alternatives = Vec::new();
    for bucket in routes {
        for guard in &bucket.alternatives {
            let guard = match guard {
                CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                    CrashPredicateTerm::new(substitute(predicate.proposition(), actuals, id)?),
                ),
            };
            alternatives.push((bucket.cause, guard));
        }
    }
    Ok(self::routes(alternatives))
}

/// Restate a clause with the same checked traversal used for call obligations.
/// The bindings replace scalar formal identities simultaneously; they cannot
/// resolve a structural observation without a captured entry value.
pub(crate) fn substitute(
    proposition: &Proposition,
    actuals: &[ValueId],
    id: MachineId,
) -> Checked<Proposition> {
    let substituted = terminal_semantics::substitution::clauses::substitute_clause(
        proposition,
        &mut ScalarFormals(actuals),
    )
    .map_err(|detail| refuse(id, Construct::Machine, Unserved::Contract(detail)))?;
    canonical_clause(substituted, id)
}

/// Substitution can reverse equality order or make conjuncts identical. Restore
/// the artifact's canonical form without changing the retained clause's meaning.
fn canonical_clause(proposition: Proposition, id: MachineId) -> Checked<Proposition> {
    match proposition {
        Proposition::Equal(left, right) => equal(left, right, id),
        Proposition::IntegerMathEqual(left, right) if left > right => {
            Ok(Proposition::IntegerMathEqual(right, left))
        }
        Proposition::Conjunction(parts) => {
            let parts = parts
                .into_iter()
                .map(|part| canonical_clause(part, id))
                .collect::<Checked<Vec<_>>>()?;
            conjunction(parts, id)
        }
        Proposition::Disjunction(parts) => {
            let parts = parts
                .into_iter()
                .map(|part| canonical_clause(part, id))
                .collect::<Checked<Vec<_>>>()?;
            let mut parts = clauses(parts, Repeated::Merge, id)?;
            Ok(match parts.len() {
                0 => Proposition::Falsehood,
                1 => parts.remove(0),
                _ => Proposition::Disjunction(parts),
            })
        }
        other => Ok(other),
    }
}

struct ScalarFormals<'a>(&'a [ValueId]);

impl terminal_semantics::substitution::clauses::ClauseBindings for ScalarFormals<'_> {
    type Error = &'static str;

    fn value(
        &mut self,
        formal: ValueId,
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let actual = usize::try_from(formal.get())
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.0.get(position))
            .ok_or("clause names a value that is not a formal")?;
        Ok(ScalarTerm::value(*actual, scalar_type))
    }

    fn field(
        &mut self,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        Err("clause restatement has no captured structural entry value")
    }

    fn case(
        &mut self,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("clause restatement has no captured structural entry case")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, ScalarType};

    #[test]
    fn clause_substitution_preserves_nested_operations_and_actual_identity() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar = ScalarType::Integer(integer);
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let clause = Proposition::IntegerMathLessThan(
            IntegerMathTerm::embed(ScalarTerm::WrappingIntegerAdd {
                scalar_type: integer,
                left: Box::new(value(1)),
                right: Box::new(value(2)),
            })
            .unwrap(),
            IntegerMathTerm::embed(value(2)).unwrap(),
        );
        let actuals = [ValueId::new(8).unwrap(), ValueId::new(3).unwrap()];
        let mut expected = clause.clone();
        expected.remap_value_ids(|formal| actuals[(formal.get() - 1) as usize]);
        assert_eq!(
            substitute(&clause, &actuals, MachineId::new(1).unwrap()).unwrap(),
            expected
        );
        assert!(substitute(&clause, &actuals[..1], MachineId::new(1).unwrap()).is_err());
    }

    #[test]
    fn clause_substitution_reorders_equalities_without_rewriting_their_operands() {
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let mathematical = |raw| IntegerMathTerm::embed(value(raw)).unwrap();
        let clause = Proposition::Conjunction(vec![
            Proposition::Equal(value(1), value(2)),
            Proposition::IntegerMathEqual(mathematical(1), mathematical(2)),
        ]);
        let actuals = [ValueId::new(8).unwrap(), ValueId::new(3).unwrap()];
        let machine = MachineId::new(1).unwrap();
        assert_eq!(
            substitute(&clause, &actuals, machine).unwrap(),
            conjunction(
                vec![
                    equal(value(3), value(8), machine).unwrap(),
                    Proposition::IntegerMathEqual(mathematical(3), mathematical(8)),
                ],
                machine,
            )
            .unwrap()
        );
    }
}
