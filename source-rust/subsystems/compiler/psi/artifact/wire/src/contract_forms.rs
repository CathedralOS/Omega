//! Canonical forms shared by contract production and receiving checks.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, CrashCause, PlaceId, Proposition, ScalarReadProjection,
    ScalarTerm, ValueId,
};
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

/// The storage a structural formal stands for in a restated clause: a place
/// of the restating scope and the path below it that the actual projects.
/// A whole actual (`@items`) has an empty prefix; a field actual
/// (`@pair.items`) keeps `items`, so a clause term `items[index]` becomes
/// `pair.items[index]` and names only that field's storage. An element
/// chosen at run time (`@rows[row]`) keeps its selector as an index over the
/// restating scope's value: `items[0]` becomes `rows[row][0]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormalStorage {
    pub root: PlaceId,
    pub prefix: Vec<ScalarReadProjection>,
    /// A simple renaming, or a readable borrow of the same whole storage.
    /// A transported value may occupy a new home; a projection can change
    /// the layout that caps an observed alignment.
    same_location_view: bool,
}

impl FormalStorage {
    pub fn whole(root: PlaceId) -> Self {
        Self {
            root,
            prefix: Vec::new(),
            same_location_view: true,
        }
    }
}

/// Where each structural actual of a call stands in the caller. A projected
/// actual keeps its exact path below its root, each element chosen at run
/// time as an index over its selector (`value_type` names the selector's
/// type). One whose path has no canonical form stands for no storage, so a
/// clause that names its formal is refused by name.
pub fn call_storage(
    types: &[terminal_psi::StructuralTypeDeclaration],
    root_type: impl Fn(PlaceId) -> Option<semantic_vocabulary::StructuralTypeId>,
    value_type: impl Fn(ValueId) -> Option<semantic_vocabulary::ScalarType>,
    structural: &[terminal_psi::StructuralArgument],
) -> Vec<Option<FormalStorage>> {
    structural
        .iter()
        .map(|argument| {
            if argument.path.is_empty() {
                return Some(FormalStorage {
                    root: argument.place,
                    prefix: Vec::new(),
                    same_location_view: matches!(
                        argument.access,
                        terminal_psi::reading_borrow_pair_pattern!()
                    ),
                });
            }
            let (prefix, _) = terminal_semantics::canonical_read_projection(
                types.iter(),
                root_type(argument.place)?,
                &argument.path,
                &value_type,
            )?;
            Some(FormalStorage {
                root: argument.place,
                same_location_view: false,
                prefix,
            })
        })
        .collect()
}

/// Retain every published crash alternative under simultaneous actuals.
/// This operation does not prove a route impossible or remove one.
///
/// A guard over a structural formal (`items[index] == 0`) names the storage
/// the call's structural actual in that lane stands for (`call_storage`),
/// read when the call is made. An actual with no canonical path has no
/// storage to name, so a guard over that formal is refused rather than dropped: an
/// untranslated continuation cannot erase a crash (calls_and_outcomes.md,
/// "Call contracts").
pub fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
    places: &[Option<FormalStorage>],
) -> Result<Vec<CrashRouteBucket>, String> {
    let mut alternatives = Vec::new();
    for bucket in routes {
        for guard in &bucket.alternatives {
            let guard = match guard {
                CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                CrashRouteGuard::Predicate(predicate) => {
                    CrashRouteGuard::Predicate(CrashPredicateTerm::new(substitute_formals(
                        predicate.proposition(),
                        actuals,
                        places,
                        &mut CallActuals,
                    )?))
                }
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
/// not facts introduced by this step. `places` holds the storage each
/// structural formal stands for, or `None` when its actual has none to name.
/// A crash continuation and a stage 05 loop header or record cast restate
/// their clauses through this one binding.
pub fn substitute_formals(
    proposition: &Proposition,
    actuals: &[ValueId],
    places: &[Option<FormalStorage>],
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
    /// The storage each structural formal stands for, or `None` when its
    /// actual has none to name.
    places: &'a [Option<FormalStorage>],
    sources: &'a mut S,
}

impl<S> ScalarFormals<'_, S> {
    fn place(&self, formal: PlaceId) -> Result<&FormalStorage, &'static str> {
        usize::try_from(formal.get())
            .ok()
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.places.get(position))
            .ok_or("clause names storage that is not a structural formal")?
            .as_ref()
            .ok_or("clause names a structural formal whose actual has no canonical path")
    }

    /// The formal's element read restated below the storage its actual
    /// stands for: the actual's prefix first, then the formal's own steps.
    fn read_path(
        &self,
        formal: PlaceId,
        path: &[ScalarReadProjection],
    ) -> Result<(PlaceId, Vec<ScalarReadProjection>), &'static str> {
        let storage = self.place(formal)?;
        let mut restated = Vec::with_capacity(storage.prefix.len() + path.len());
        restated.extend_from_slice(&storage.prefix);
        restated.extend_from_slice(path);
        Ok((storage.root, restated))
    }

    /// The formal's static path restated below its actual's storage, for a
    /// field, extent or byte predicate. `None` when the actual selects an
    /// element at run time, which no static path can name.
    fn static_path(
        &self,
        formal: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>, &'static str> {
        let storage = self.place(formal)?;
        let Some(mut restated) = storage
            .prefix
            .iter()
            .map(|step| match step {
                ScalarReadProjection::Static(segment) => Some(*segment),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        restated.extend_from_slice(path);
        Ok(Some((storage.root, restated)))
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
        let Some((root, path)) = self.static_path(root, path)? else {
            // Below an element chosen at run time the field is that
            // element's read: `p.a` over `@rows[row]` is `rows[row].a`.
            let path = path
                .iter()
                .copied()
                .map(ScalarReadProjection::from)
                .collect::<Vec<_>>();
            let (root, path) = self.read_path(root, &path)?;
            return Ok(ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            });
        };
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
        path: &[ScalarReadProjection],
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        // Substitution already renamed each selector into the consumer's
        // scope. The restated clause names the observation; the checker reads
        // that storage where the call is made or the edge arrives.
        if matches!(path.last(), Some(ScalarReadProjection::Alignment))
            && !self.place(root)?.same_location_view
        {
            return Err("restated alignment requires the same storage and declared geometry");
        }
        let (root, path) = self.read_path(root, path)?;
        Ok(ScalarTerm::StructuralRead {
            root,
            path,
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
        let (root, path) = self
            .static_path(root, path)?
            .ok_or("a byte predicate names storage below an element chosen at run time")?;
        Ok(ScalarTerm::byte_sequence_predicate(predicate, root, path))
    }

    fn extent(
        &mut self,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        let (root, path) = self
            .static_path(root, path)?
            .ok_or("an extent names storage below an element chosen at run time")?;
        Ok(ScalarTerm::ViewExtent {
            root,
            path,
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

    #[test]
    fn restating_location_alignment_requires_the_same_storage_view() {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        use terminal_psi::{StructuralAccess, StructuralArgument};
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar = ScalarType::Integer(integer);
        let source = PlaceId::new(1).unwrap();
        let target = PlaceId::new(9).unwrap();
        let clause = Proposition::Equal(
            ScalarTerm::structural_read(source, vec![ScalarReadProjection::Alignment], scalar)
                .unwrap(),
            ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(8),
            },
        );
        for (access, shares_home) in [
            (StructuralAccess::SharedBorrow, true),
            (StructuralAccess::MutableBorrow, true),
            (StructuralAccess::WriteOnlyBorrow, false),
            (StructuralAccess::Owned, false),
        ] {
            let places = call_storage(
                &[],
                |_| None,
                |_| None,
                &[StructuralArgument {
                    place: target,
                    path: Vec::new(),
                    access,
                }],
            );
            let result = substitute_formals(&clause, &[], &places, &mut CallActuals);
            assert_eq!(result.is_ok(), shares_home, "{access:?}: {result:?}");
        }
    }

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

    /// A guard `items[i] == i` over a formal lent `@pair.items` names the
    /// element below the actual's field: `pair.items[i]`, with the field
    /// segment before the selector.
    #[test]
    fn a_projected_actual_prefixes_its_static_path() {
        use semantic_vocabulary::{IntegerSign, IntegerType, StructuralFieldId};
        let ty = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ty);
        let index = |id| ScalarReadProjection::Index(Box::new(value(id)));
        let guard = |root, path: Vec<ScalarReadProjection>, selector| {
            Proposition::Equal(
                ScalarTerm::StructuralRead {
                    root: PlaceId::new(root).unwrap(),
                    path,
                    scalar_type: ty,
                },
                value(selector),
            )
        };
        let field = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(2).unwrap());
        let places = [Some(FormalStorage {
            same_location_view: false,
            root: PlaceId::new(5).unwrap(),
            prefix: vec![ScalarReadProjection::Static(field)],
        })];
        let actuals = [ValueId::new(9).unwrap()];
        let formal = guard(1, vec![index(1)], 1);
        let restated = substitute_formals(&formal, &actuals, &places, &mut CallActuals).unwrap();
        assert_eq!(
            restated,
            canonical_clause(guard(
                5,
                vec![ScalarReadProjection::Static(field), index(9)],
                9
            ))
            .unwrap()
        );
        assert!(
            substitute_formals(&formal, &actuals, &[None], &mut CallActuals)
                .unwrap_err()
                .contains("no canonical path")
        );
    }

    /// An actual that lends an element chosen at run time (`@rows[row]`)
    /// keeps its selector: the guard `items[i] == i` names `rows[row][i]`,
    /// and a field guard below it, `p.a == 0`, names the read `rows[row].a`
    /// because no static path reaches it.
    #[test]
    fn a_runtime_selected_actual_prefixes_its_selector() {
        use semantic_vocabulary::{IntegerSign, IntegerType, StructuralFieldId};
        let ty = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ty);
        let index = |id| ScalarReadProjection::Index(Box::new(value(id)));
        let read = |root, path: Vec<ScalarReadProjection>| ScalarTerm::StructuralRead {
            root: PlaceId::new(root).unwrap(),
            path,
            scalar_type: ty,
        };
        let places = [Some(FormalStorage {
            same_location_view: false,
            root: PlaceId::new(5).unwrap(),
            prefix: vec![index(7)],
        })];
        let actuals = [ValueId::new(9).unwrap()];
        let element = Proposition::Equal(read(1, vec![index(1)]), value(1));
        assert_eq!(
            substitute_formals(&element, &actuals, &places, &mut CallActuals).unwrap(),
            canonical_clause(Proposition::Equal(
                read(5, vec![index(7), index(9)]),
                value(9)
            ))
            .unwrap()
        );
        let field = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(2).unwrap());
        let zero = ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            semantic_vocabulary::IntegerValue::Unsigned(0),
        )
        .unwrap();
        let field_guard = Proposition::Equal(
            ScalarTerm::IntegerField {
                version: semantic_vocabulary::ContentPlaceVersion::Current,
                root: PlaceId::new(1).unwrap(),
                path: vec![field],
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            },
            zero.clone(),
        );
        assert_eq!(
            substitute_formals(&field_guard, &actuals, &places, &mut CallActuals).unwrap(),
            canonical_clause(Proposition::Equal(
                read(5, vec![index(7), ScalarReadProjection::Static(field)]),
                zero
            ))
            .unwrap()
        );
    }
}
