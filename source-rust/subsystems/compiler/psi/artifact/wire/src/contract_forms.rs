//! Canonical forms shared by contract production and receiving checks.

use semantic_vocabulary::{CrashCause, Proposition, ScalarTerm, ValueId};
use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

#[derive(Clone, Copy)]
pub enum RepeatedClauses {
    Refuse,
    Merge,
}

pub fn equal(left: ScalarTerm, right: ScalarTerm) -> Result<Proposition, String> {
    if left.scalar_type() == right.scalar_type()
        && [&left, &right]
            .into_iter()
            .any(|term| matches!(term, ScalarTerm::DeclarationApplication { .. }))
    {
        use semantic_vocabulary::{ClauseCarrier, ClauseIdentity, ClauseOperand, ClauseValue};
        let carrier = ClauseCarrier::Scalar(left.scalar_type());
        let values = [left, right]
            .into_iter()
            .map(|term| ClauseValue {
                carrier,
                operand: ClauseOperand::Scalar(term),
            })
            .collect();
        return Ok(Proposition::DeclarationIdentity(ClauseIdentity {
            values,
            left: 0,
            right: 1,
        }));
    }

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
///
/// A guard over a structural formal (`items[index] == 0`) names the place of
/// the call's structural actual in that lane, read when the call is made. An
/// actual that projects its place (`@pair.items`) has no place of its own to
/// name, so a guard over that formal is refused rather than dropped: an
/// untranslated continuation cannot erase a crash (calls_and_outcomes.md,
/// "Call contracts").
pub fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
    structural: &[terminal_psi::StructuralArgument],
) -> Result<Vec<CrashRouteBucket>, String> {
    let places: Vec<_> = structural
        .iter()
        .map(|argument| argument.path.is_empty().then_some(argument.place))
        .collect();
    let mut alternatives = Vec::new();
    for bucket in routes {
        for guard in &bucket.alternatives {
            let guard = match guard {
                CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                    CrashPredicateTerm::new(substitute_formals(
                        predicate.proposition(),
                        actuals,
                        &places,
                        &mut CallActuals,
                    )?),
                ),
            };
            alternatives.push((bucket.cause, guard));
        }
    }
    Ok(self::routes(alternatives))
}

pub fn substitute(proposition: &Proposition, actuals: &[ValueId]) -> Result<Proposition, String> {
    substitute_formals(proposition, actuals, &[], &mut CallActuals)
}

/// What a restated clause may keep beyond the renamed formals. Each consumer
/// says which sources it can trace; the defaults refuse, because a consumer
/// that cannot trace a source must not invent one.
pub trait RestatedSources {
    /// Accept the source of a float meaning projection
    /// (`float_conversion_roundtrip(x, ..)`) after renaming.
    fn float_meaning_source(&mut self, _source: &ScalarTerm) -> Result<(), &'static str> {
        Err("float meaning projection has no actual owner correspondence")
    }

    /// Whether a byte predicate over a structural formal keeps its renamed
    /// place. A consumer that proves the restated clause at each arrival can
    /// keep it; a call's crash continuation cannot.
    fn keeps_byte_predicates(&self) -> bool {
        false
    }
}

/// A call's crash continuation: it renames formals and traces no other source.
struct CallActuals;

impl RestatedSources for CallActuals {}

/// Substitute exact scalar and storage formal identities together, then
/// restore canonical order. Storage observations remain predicates to prove,
/// not facts introduced by this step. `places` holds the place each structural
/// formal stands for, or `None` when its actual projects a place. A crash
/// continuation and a stage 05 loop header or record cast restate their
/// clauses through this one binding.
pub fn substitute_formals(
    proposition: &Proposition,
    actuals: &[ValueId],
    places: &[Option<semantic_vocabulary::PlaceId>],
    sources: &mut impl RestatedSources,
) -> Result<Proposition, String> {
    let substituted = terminal_semantics::substitution::clauses::substitute_clause(
        proposition,
        &mut ScalarFormals {
            values: actuals,
            places,
            sources,
        },
    )
    .map_err(str::to_owned)?;
    canonical_clause(substituted)
}

/// Substitution can reverse equality order or make conjuncts identical, at
/// any depth. Restore the artifact's canonical form without changing the
/// clause's meaning; source lowering and call restatement share this one form.
pub fn canonical_clause(proposition: Proposition) -> Result<Proposition, String> {
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
        Proposition::Implication {
            premise,
            conclusion,
        } => Ok(Proposition::Implication {
            premise: Box::new(canonical_clause(*premise)?),
            conclusion: Box::new(canonical_clause(*conclusion)?),
        }),
        other => Ok(other),
    }
}

struct ScalarFormals<'a, S> {
    values: &'a [ValueId],
    /// The place each structural formal stands for, or `None` when its actual
    /// projects a place.
    places: &'a [Option<semantic_vocabulary::PlaceId>],
    sources: &'a mut S,
}

impl<S> ScalarFormals<'_, S> {
    fn place(
        &self,
        formal: semantic_vocabulary::PlaceId,
    ) -> Result<semantic_vocabulary::PlaceId, &'static str> {
        usize::try_from(formal.get())
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.places.get(position))
            .ok_or("clause names storage that is not a structural formal")?
            .ok_or("clause names a structural formal whose actual projects a place")
    }
}

impl<S: RestatedSources> terminal_semantics::substitution::clauses::ClauseBindings
    for ScalarFormals<'_, S>
{
    type Error = &'static str;

    fn value(
        &mut self,
        formal: ValueId,
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let actual = usize::try_from(formal.get())
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.values.get(position))
            .ok_or("clause names a value that is not a formal")?;
        Ok(ScalarTerm::value(*actual, scalar_type))
    }

    fn field(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        if version != semantic_vocabulary::ContentPlaceVersion::Current {
            return Err("a restated clause cannot name a prior invocation's storage snapshot");
        }
        let root = self.place(root)?;
        let path = path.to_vec();
        Ok(match scalar_type {
            semantic_vocabulary::ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            },
            semantic_vocabulary::ScalarType::Boolean => ScalarTerm::BooleanField {
                version,
                root,
                path,
            },
            semantic_vocabulary::ScalarType::IeeeFloat(format) => ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            },
            semantic_vocabulary::ScalarType::Callable(_) => {
                return Err("a callable field has no scalar contract term");
            }
        })
    }

    fn structural_read(
        &mut self,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::ScalarReadProjection],
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        // Substitution already renamed each selector into the consumer's
        // scope. The restated clause names the observation; the checker reads
        // that storage where the call is made or the edge arrives.
        Ok(ScalarTerm::StructuralRead {
            root: self.place(root)?,
            path: path.to_vec(),
            scalar_type,
        })
    }

    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Self::Error> {
        self.sources.float_meaning_source(source)
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        if !self.sources.keeps_byte_predicates() {
            return Err("clause byte predicate has no byte source observation");
        }
        Ok(ScalarTerm::byte_sequence_predicate(
            predicate,
            self.place(root)?,
            path.to_vec(),
        ))
    }

    fn extent(
        &mut self,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::ViewExtent {
            root: self.place(root)?,
            path: path.to_vec(),
            scalar_type,
        })
    }

    fn case(
        &mut self,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::StructuralCaseId,
    ) -> Result<terminal_semantics::substitution::clauses::CaseAnswer, Self::Error> {
        Err("clause restatement has no captured structural entry case")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_declaration_encoding::terms::order::validate_canonical_proposition;
    use semantic_vocabulary::ScalarType;

    /// A callee's guard `x == y -> x == y` restated with actuals whose IDs
    /// sort the other way must still be in canonical wire order, including
    /// inside an implication.
    #[test]
    fn substitution_restores_equality_order_inside_an_implication() {
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Boolean);
        let implication = |left, right| Proposition::Implication {
            premise: Box::new(Proposition::Equal(value(left), value(right))),
            conclusion: Box::new(Proposition::Equal(value(left), value(right))),
        };
        let formal = implication(1, 2);
        validate_canonical_proposition(&formal).unwrap();
        let actuals = [ValueId::new(9).unwrap(), ValueId::new(3).unwrap()];
        let restated = substitute(&formal, &actuals).unwrap();
        validate_canonical_proposition(&restated).unwrap();
        assert_eq!(restated, implication(3, 9));
    }
}
