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
    let mut keyed = Vec::with_capacity(propositions.len());
    for proposition in propositions {
        let key = terminal_codec::canonical_proposition_order_key(&proposition).map_err(|_| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("unencodable proposition"),
            )
        })?;
        keyed.push((key, proposition));
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
        .map(|(_, proposition)| proposition)
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
pub fn substituted_routes(
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

/// A clause over a machine's formals with each formal replaced by the value
/// at its position: formal `i` is the machine's value `i + 1`.
pub(crate) fn substitute(
    proposition: &Proposition,
    actuals: &[ValueId],
    id: MachineId,
) -> Checked<Proposition> {
    let term = |term: &ScalarTerm| substitute_term(term, actuals, id);
    match proposition {
        Proposition::Truth => Ok(Proposition::Truth),
        Proposition::Falsehood => Ok(Proposition::Falsehood),
        Proposition::Equal(left, right) => equal(term(left)?, term(right)?, id),
        Proposition::LessThan(left, right) => Ok(Proposition::LessThan(term(left)?, term(right)?)),
        Proposition::LessOrEqual(left, right) => {
            Ok(Proposition::LessOrEqual(term(left)?, term(right)?))
        }
        Proposition::Conjunction(conjuncts) => {
            let mut substituted = Vec::with_capacity(conjuncts.len());
            for conjunct in conjuncts {
                substituted.push(substitute(conjunct, actuals, id)?);
            }
            conjunction(substituted, id)
        }
        // A machine's `requires` may compare mathematical integers, and a
        // member of a call cycle has its `requires` restated here as its
        // head's invariant. Restating one needs each formal's block
        // parameter embedded where the formal was, and the checker's
        // arrival goals to be stated the same way; neither is built.
        Proposition::IntegerMathEqual(..)
        | Proposition::IntegerMathLessThan(..)
        | Proposition::IntegerMathLessOrEqual(..) => Err(refuse(
            id,
            Construct::Machine,
            Unserved::Contract("mathematical comparison required on a call cycle"),
        )),
        // The reader builds route guards only from the forms above.
        Proposition::Atom(_)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ScalarIeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralIdentity { .. }
        | Proposition::StructuralCaseMembership { .. }
        | Proposition::Disjunction(_)
        | Proposition::Implication { .. }
        | Proposition::ContentConservation(_) => Err(refuse(
            id,
            Construct::Machine,
            Unserved::Contract("route guard outside the scalar comparisons"),
        )),
    }
}

/// Formal `i` is the callee's value `i + 1` (`Signature::parameters`); a route
/// guard names nothing else but literals.
fn substitute_term(term: &ScalarTerm, actuals: &[ValueId], id: MachineId) -> Checked<ScalarTerm> {
    match term {
        ScalarTerm::Value {
            id: formal,
            scalar_type,
        } => {
            let actual = usize::try_from(formal.get())
                .ok()
                .and_then(|position| position.checked_sub(1))
                .and_then(|position| actuals.get(position))
                .ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("route guard names a value that is not a formal"),
                    )
                })?;
            Ok(ScalarTerm::Value {
                id: *actual,
                scalar_type: *scalar_type,
            })
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => Ok(term.clone()),
        _ => Err(refuse(
            id,
            Construct::Machine,
            Unserved::Contract("route guard term outside formals and literals"),
        )),
    }
}
