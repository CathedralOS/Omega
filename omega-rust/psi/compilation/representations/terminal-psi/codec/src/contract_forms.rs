//! Canonical forms shared by contract production and receiving checks.

use semantic_vocabulary::{CrashCause, Proposition, ScalarTerm, ValueId};
use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

#[derive(Clone, Copy)]
pub enum RepeatedClauses {
    Refuse,
    Merge,
}

pub fn equal(left: ScalarTerm, right: ScalarTerm) -> Result<Proposition, String> {
    let key = |term: &ScalarTerm| {
        crate::canonical_scalar_term_order_key(term)
            .map_err(|error| format!("unencodable term: {error}"))
    };
    if key(&left)? <= key(&right)? {
        Ok(Proposition::Equal(left, right))
    } else {
        Ok(Proposition::Equal(right, left))
    }
}

pub fn clauses(
    propositions: Vec<Proposition>,
    repeated: RepeatedClauses,
) -> Result<Vec<Proposition>, String> {
    let mut keyed = Vec::with_capacity(propositions.len());
    for proposition in propositions {
        let key = crate::canonical_proposition_order_key(&proposition)
            .map_err(|error| format!("unencodable proposition: {error}"))?;
        keyed.push((key, proposition));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    if matches!(repeated, RepeatedClauses::Refuse)
        && keyed.windows(2).any(|pair| pair[0].0 == pair[1].0)
    {
        return Err("duplicate clause".into());
    }
    keyed.dedup_by(|later, earlier| later.0 == earlier.0);
    Ok(keyed
        .into_iter()
        .map(|(_, proposition)| proposition)
        .collect())
}

pub fn conjunction(conjuncts: Vec<Proposition>) -> Result<Proposition, String> {
    let mut flat = Vec::with_capacity(conjuncts.len());
    for conjunct in conjuncts {
        match conjunct {
            Proposition::Conjunction(inner) => flat.extend(inner),
            other => flat.push(other),
        }
    }
    let mut ordered = clauses(flat, RepeatedClauses::Merge)?;
    Ok(match ordered.len() {
        0 => Proposition::Truth,
        1 => ordered.remove(0),
        _ => Proposition::Conjunction(ordered),
    })
}

pub fn routes(
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

/// Retain every published crash alternative under simultaneous actuals.
/// This operation does not prove a route impossible or remove one.
pub fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
) -> Result<Vec<CrashRouteBucket>, String> {
    let mut alternatives = Vec::new();
    for bucket in routes {
        for guard in &bucket.alternatives {
            let guard = match guard {
                CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                    CrashPredicateTerm::new(substitute(predicate.proposition(), actuals)?),
                ),
            };
            alternatives.push((bucket.cause, guard));
        }
    }
    Ok(self::routes(alternatives))
}

pub fn substitute(proposition: &Proposition, actuals: &[ValueId]) -> Result<Proposition, String> {
    let substituted = terminal_semantics::substitution::clauses::substitute_clause(
        proposition,
        &mut ScalarFormals(actuals),
    )
    .map_err(str::to_owned)?;
    canonical_clause(substituted)
}

fn canonical_clause(proposition: Proposition) -> Result<Proposition, String> {
    match proposition {
        Proposition::Equal(left, right) => equal(left, right),
        Proposition::IntegerMathEqual(left, right) if left > right => {
            Ok(Proposition::IntegerMathEqual(right, left))
        }
        Proposition::Conjunction(parts) => conjunction(
            parts
                .into_iter()
                .map(canonical_clause)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Proposition::Disjunction(parts) => {
            let parts = parts
                .into_iter()
                .map(canonical_clause)
                .collect::<Result<Vec<_>, _>>()?;
            let mut parts = clauses(parts, RepeatedClauses::Merge)?;
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
