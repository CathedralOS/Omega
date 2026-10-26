//! Prepare the live membership subjects of one immutable evidence selection.
//!
//! Call and return field checks ask thousands of questions against the same
//! namespace. Resolve each live subject once, then search exact coordinates.
//! Extent and runtime-selector rows retain the original coverage judgment;
//! structural equality alone cannot authorize an unresolved field or case.
//! This is an invocation-local observation plan, not a declaration roster or
//! cached proof: it cannot recover a retired fact or combine capture epochs.

use crate::CanonicalPlace;
use facts::{FactContextHandle, FactPlan, PlaceSegment};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

struct Membership {
    place: CanonicalPlace,
    domain: SymbolHandle,
    instance: SemanticDomainId,
}

pub struct MembershipEvidence<'checking> {
    program: &'checking dyn crate::CheckingInput,
    // Pin the immutable evidence owner while its prepared subjects are queried.
    _semantic: &'checking FactPlan,
    rows: Vec<Membership>,
    flexible: Vec<usize>,
}

impl<'checking> MembershipEvidence<'checking> {
    pub fn prepare(
        program: &'checking dyn crate::CheckingInput,
        semantic: &'checking FactPlan,
        contexts: &[FactContextHandle],
    ) -> Self {
        let mut rows = Vec::new();
        for handle in semantic.unique_context_facts(contexts.iter().copied()) {
            let fact = semantic.facts.get(handle);
            let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                continue;
            };
            let Some(place) = fact.place.as_place().and_then(|place| {
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
            }) else {
                continue;
            };
            rows.push(Membership {
                place,
                domain,
                instance,
            });
        }
        rows.sort_unstable_by(|left, right| {
            left.place.compare_identity(&right.place).then_with(|| {
                domain_key(left.domain, left.instance)
                    .cmp(&domain_key(right.domain, right.instance))
            })
        });
        let flexible = rows
            .iter()
            .enumerate()
            .filter_map(|(position, row)| {
                row.place
                    .segments
                    .iter()
                    .any(|segment| {
                        matches!(
                            segment,
                            PlaceSegment::Index { .. } | PlaceSegment::FixedRange { .. }
                        )
                    })
                    .then_some(position)
            })
            .collect();
        Self {
            program,
            _semantic: semantic,
            rows,
            flexible,
        }
    }

    pub fn contains(
        &self,
        subject: &CanonicalPlace,
        domain: SymbolHandle,
        instance: SemanticDomainId,
    ) -> bool {
        if !instance.is_valid() {
            return false;
        }
        let matches = |row: &Membership| {
            row.domain == domain
                && row.instance == instance
                && row.place.root == subject.root
                && segments_cover_subject(self.program, &row.place.segments, &subject.segments)
        };
        if self
            .rows
            .binary_search_by(|row| {
                row.place.compare_identity(subject).then_with(|| {
                    domain_key(row.domain, row.instance).cmp(&domain_key(domain, instance))
                })
            })
            .ok()
            .is_some_and(|position| matches(&self.rows[position]))
        {
            return true;
        }
        if subject
            .segments
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
        {
            // A separately spelled selector can equal another expression or a
            // fixed index. Handle order does not decide structural equality.
            self.rows.iter().any(matches)
        } else {
            self.flexible
                .iter()
                .any(|position| matches(&self.rows[*position]))
        }
    }
}

fn domain_key(domain: SymbolHandle, instance: SemanticDomainId) -> (u32, u32, u32) {
    (domain.arena_index(), domain.generation(), instance.0)
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
pub fn segments_cover_subject(
    program: &dyn crate::CheckingInput,
    candidate: &[facts::PlaceSegment],
    subject: &[facts::PlaceSegment],
) -> bool {
    candidate.len() == subject.len()
        && candidate
            .iter()
            .zip(subject.iter())
            .all(|(candidate, subject)| {
                crate::storage_path_relations::canonical_place_segments_equal(
                    program, *candidate, *subject,
                ) || segment_covers_subject(program, *candidate, *subject)
            })
}

fn segment_covers_subject(
    program: &dyn crate::CheckingInput,
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

pub fn canonical_place_from_semantic_place(
    program: &dyn crate::CheckingInput,
    semantic: &FactPlan,
    place: &facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        facts::PlaceRoot::Unknown => return None,
        facts::PlaceRoot::Symbol(symbol) => symbol.is_valid().then_some(CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        })?,
        root @ (facts::PlaceRoot::InvocationReferent { .. }
        | facts::PlaceRoot::InvocationOutput(_)) => CanonicalPlace {
            root,
            segments: Vec::new(),
        },
        facts::PlaceRoot::Expression(expression) => {
            // Semantic expression roots name completed values. Replaying their
            // syntax would replace that identity with newer source storage.
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            }
        }
        facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
}
