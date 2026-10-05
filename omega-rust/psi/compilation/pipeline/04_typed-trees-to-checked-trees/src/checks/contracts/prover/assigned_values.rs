//! Domain predicates over exact, live assignment values. Mutation invalidation
//! owns their lifetime; this consumer neither replays a body nor assumes zeros.

use crate::flow::{
    CanonicalPlace, canonical_place_from_semantic_place, relative_place_segments_from_expression,
};
use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan, PlaceHandle};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;

use super::scalars::{self, ScalarValue};
use crate::values::literal_at_place;
pub(in crate::checks::contracts) use crate::values::scalar_value_at_place;

pub(super) fn prove_domain(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    place: PlaceHandle,
    domain: SymbolHandle,
) -> bool {
    let Some(subject) =
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
    else {
        return false;
    };
    prove_domain_at_place(program, semantic, contexts, &subject, domain)
}

/// Whether a fact's segment chain covers the subject's elementwise: every
/// position equal under `canonical_place_segments_equal`, or the candidate's
/// `FixedRange` extent contains the subject's selector. `x[a..b]` covers a
/// literal `x[i]` when `a <= i < b`, a narrower `x[c..d]` when `a <= c` and
/// `d <= b`, and -- ONLY at the whole-extent `0..usize::MAX` row -- a runtime
/// `x[index]` (whichever element the selector names sits inside the complete
/// extent). A runtime selector is never itself a fact coordinate, and any
/// element write retires the covering row through the usual overlap
/// machinery before it can be consulted, so coverage cannot outlive its
/// evidence.
pub(in crate::checks::contracts) fn segments_cover_subject(
    program: &TypedTrees,
    candidate: &[facts::PlaceSegment],
    subject: &[facts::PlaceSegment],
) -> bool {
    candidate.len() == subject.len()
        && candidate
            .iter()
            .zip(subject.iter())
            .all(|(candidate, subject)| {
                crate::flow::canonical_place_segments_equal(program, *candidate, *subject)
                    || segment_covers_subject(program, *candidate, *subject)
            })
}

fn segment_covers_subject(
    program: &TypedTrees,
    candidate: facts::PlaceSegment,
    subject: facts::PlaceSegment,
) -> bool {
    let facts::PlaceSegment::FixedRange { start, end } = candidate else {
        return false;
    };
    if start >= end {
        return false;
    }
    match subject {
        facts::PlaceSegment::FixedIndex { index } => start <= index && index < end,
        facts::PlaceSegment::FixedRange {
            start: subject_start,
            end: subject_end,
        } => subject_start < subject_end && start <= subject_start && subject_end <= end,
        facts::PlaceSegment::Index { expression } => {
            // Only the complete extent covers an unresolved selector. A
            // constant expression narrows from any containing extent; a
            // non-literal selector is universal only inside `0..usize::MAX`.
            if start == 0 && end == usize::MAX {
                true
            } else {
                program
                    .expression_table
                    .constant_integer_value(expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some_and(|index| start <= index && index < end)
            }
        }
        _ => false,
    }
}

/// A subject carrying a `FixedRange` window is covered elementwise: prove the
/// domain at every index inside the extent. A finite `x[a..b]` enumerates its
/// bounds directly. The whole-extent `x[0..usize::MAX]` row enumerates the
/// index set the seeding recorded for that collection -- the plan's
/// `x[i].<suffix>` coordinates are exactly the fixed extent `0..N`, seeded
/// contiguously, so a contiguous record is complete element coverage; a slice
/// records no element coordinates and returns empty. Each element is then
/// proved on its own evidence: the entry row where still live, or the
/// write's re-establishment where the write was domain-carrying.
fn prove_domain_by_extent_enumeration(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    let Some(position) = subject
        .segments
        .iter()
        .position(|segment| matches!(segment, facts::PlaceSegment::FixedRange { .. }))
    else {
        return false;
    };
    let facts::PlaceSegment::FixedRange { start, end } = subject.segments[position] else {
        return false;
    };
    let indices: Vec<usize> = if start == 0 && end == usize::MAX {
        recorded_element_extent(program, semantic, subject, position)
    } else {
        (start..end).collect()
    };
    !indices.is_empty()
        && indices.iter().all(|index| {
            let mut element = subject.clone();
            element.segments[position] = facts::PlaceSegment::FixedIndex { index: *index };
            prove_domain_at_place(program, semantic, contexts, &element, domain)
        })
}

/// The element indices seeded for the collection at `position` of `subject`:
/// every fact place sharing `subject`'s root, prefix, and suffix with a
/// `FixedIndex` at `position`. Fixed-array seeding emits exactly `0..N`
/// contiguous element coordinates, so a contiguous record is the collection's
/// whole extent; a gap means the coordinates came from elsewhere and are not
/// a whole-extent claim. Never confuses a partial record with coverage.
fn recorded_element_extent(
    program: &TypedTrees,
    semantic: &FactPlan,
    subject: &CanonicalPlace,
    position: usize,
) -> Vec<usize> {
    let mut indices = Vec::new();
    for (_, fact) in semantic.facts.iter() {
        let FactPlace::Place(place) = fact.place else {
            continue;
        };
        let Some(candidate) =
            canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
        else {
            continue;
        };
        if (candidate.root) != (subject.root)
            || candidate.segments.len() != subject.segments.len()
            || !candidate
                .segments
                .iter()
                .zip(&subject.segments)
                .enumerate()
                .all(|(at, (candidate, subject))| {
                    if at == position {
                        matches!(candidate, facts::PlaceSegment::FixedIndex { .. })
                    } else {
                        crate::flow::canonical_place_segments_equal(program, *candidate, *subject)
                    }
                })
        {
            continue;
        }
        if let facts::PlaceSegment::FixedIndex { index } = candidate.segments[position] {
            indices.push(index);
        }
    }
    indices.sort_unstable();
    indices.dedup();
    if indices.last().is_some_and(|max| indices.len() == max + 1) {
        indices
    } else {
        Vec::new()
    }
}

/// Whether a carried membership fact on `subject` (a declared constraint or
/// established contract evidence) already implies `domain`. This is the
/// evidence arm of [`prove_domain_at_place`]: routed provenance and prior
/// `ensures`/`in`-constrained declarations answer membership questions that
/// predicate discharge cannot reach.
fn carried_domain_membership_at_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| {
                let candidate_domain = match fact.payload {
                    FactPayload::DomainMembership { domain_symbol, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                    _ => return false,
                };
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
                    .is_some_and(|candidate| {
                        (candidate.root) == (subject.root)
                            && segments_cover_subject(
                                program,
                                &candidate.segments,
                                &subject.segments,
                            )
                            && crate::facts::field_domain::domain_membership_implies(
                                program,
                                candidate_domain,
                                domain,
                            )
                    })
            })
    })
}

pub(in crate::checks::contracts) fn prove_domain_at_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    carried_domain_membership_at_place(program, semantic, contexts, subject, domain)
        || prove_domain_by_extent_enumeration(program, semantic, contexts, subject, domain)
        || AssignedValues {
            program,
            semantic,
            contexts,
        }
        .domain(subject, domain, &mut Vec::new())
}

struct AssignedValues<'a> {
    program: &'a TypedTrees,
    semantic: &'a FactPlan,
    contexts: &'a [FactContextHandle],
}

impl AssignedValues<'_> {
    fn literal(&self, subject: &CanonicalPlace) -> Option<ExpressionHandle> {
        literal_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            subject,
        )
    }
}

/// Whether a live per-byte class fact at `subject` implies `required`. The one
/// scan behind both consumers of minted `FactPayload::BytePredicate` evidence:
/// a domain reduced to exactly one recognized byte predicate proves
/// membership through it, and a `requires pred(place)` conjunct or a
/// `pred(self)` initializer obligation discharges against it — identical
/// place-equality plus `implies` semantics in every consumer.
pub(in crate::checks::contracts) fn byte_predicate_proven_at_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    required: crate::facts::field_domain::ByteSequencePredicate,
) -> bool {
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| {
                let FactPayload::BytePredicate { predicate } = fact.payload else {
                    return false;
                };
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                predicate.implies(required)
                    && canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    )
                    .is_some_and(|candidate| {
                        crate::flow::canonical_places_agree(program, &candidate, subject)
                    })
            })
    })
}

impl AssignedValues<'_> {
    /// Whether a live per-byte class proved for the whole carrier entails the
    /// requested domain. An indexed write retires the exact value snapshot but
    /// leaves this class behind, which is how a text carrier stays provable
    /// across `buffer[i] = byte`. A domain that is not exactly one recognized
    /// byte predicate is never reached this way.
    fn byte_predicate(&self, subject: &CanonicalPlace, symbol: SymbolHandle) -> bool {
        let Some(required) =
            crate::facts::field_domain::domain_byte_predicate(self.program, symbol)
        else {
            return false;
        };
        byte_predicate_proven_at_place(
            self.program,
            self.semantic,
            self.contexts,
            subject,
            required,
        )
    }

    fn relative_subject(
        &self,
        subject: &CanonicalPlace,
        expression: ExpressionHandle,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> Option<CanonicalPlace> {
        let segments = relative_place_segments_from_expression(self.program, expression, domain)?;
        let mut selected = subject.clone();
        selected.extend_segments(&segments);
        Some(selected)
    }

    fn domain(
        &self,
        subject: &CanonicalPlace,
        symbol: SymbolHandle,
        active: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !symbol.is_valid() || active.contains(&symbol) {
            return false;
        }
        let Some(domain) = self
            .program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
        else {
            return false;
        };
        if let Some(literal) = self.literal(subject)
            && crate::facts::field_domain::string_literal_expression_grants_domain(
                self.program,
                literal,
                symbol,
            )
        {
            return true;
        }
        if self.byte_predicate(subject, symbol) {
            return true;
        }
        if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
            return false;
        }
        let facts = self.program.proof_facts.span_or_empty(domain.facts);
        if facts.is_empty() {
            return false;
        }
        active.push(symbol);
        let proved = facts.iter().all(|fact| match fact {
            ProofFact::Expression(expression) => {
                self.boolean(subject, domain, *expression) == Some(true)
            }
            ProofFact::Membership(membership) => self
                .relative_subject(subject, membership.value, domain)
                .is_some_and(|nested| {
                    carried_domain_membership_at_place(
                        self.program,
                        self.semantic,
                        self.contexts,
                        &nested,
                        membership.domain_symbol,
                    ) || self.domain(&nested, membership.domain_symbol, active)
                }),
            ProofFact::Proposition(_) => false,
        });
        active.pop();
        proved
    }

    /// The place's installed nominal tag, when a live assignment fact has
    /// decided it. `let x = T::C{...}` mints one; nothing else does.
    fn installed_case(&self, subject: &CanonicalPlace) -> Option<SymbolHandle> {
        self.contexts.iter().find_map(|context| {
            self.semantic
                .context_view(self.semantic.contexts.get(*context))
                .facts()
                .find_map(|fact| {
                    let FactPayload::AssignedCase { variant } = fact.payload else {
                        return None;
                    };
                    let FactPlace::Place(place) = fact.place else {
                        return None;
                    };
                    let candidate = canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    )?;
                    crate::flow::canonical_places_agree(self.program, &candidate, subject)
                        .then_some(variant)
                })
        })
    }

    fn boolean(
        &self,
        subject: &CanonicalPlace,
        domain: &typed_trees::domain::DomainDefinition,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        // `self in T::C` (and the `self == T::C` equality it lowers to) names
        // a nominal tag, not a scalar comparison: the assigned case at the
        // subject's place decides it.
        if let Some((member, case)) =
            crate::proof::exact_outcome_case_test(self.program, expression)
            && let Some(selected) = self.relative_subject(subject, member, domain)
        {
            return self
                .installed_case(&selected)
                .map(|variant| variant == case);
        }
        let value = scalars::evaluate_with_comparisons(
            self.program,
            expression,
            &mut |expression| {
                // Preserve this domain prover's existing pure constant vocabulary.
                // The shared evaluator itself never picks an arithmetic domain.
                self.program
                    .expression_table
                    .constant_integer_value(expression)
                    .map(|value| ScalarValue::Integer(numerics::bignum::BigInt::from_i64(value)))
                    .or_else(|| {
                        let selected = self.relative_subject(subject, expression, domain)?;
                        scalar_value_at_place(
                            self.program,
                            self.semantic,
                            self.contexts
                                .iter()
                                .map(|context| self.semantic.contexts.get(*context)),
                            &selected,
                        )
                    })
            },
            &|left, right| {
                matches!(
                    (left, right),
                    (ScalarValue::Integer(_), ScalarValue::Integer(_))
                )
            },
        )?;
        match value {
            ScalarValue::Boolean(value) => Some(value),
            ScalarValue::Integer(_) | ScalarValue::Unknown => None,
        }
    }
}
