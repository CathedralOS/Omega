//! Prepare the live membership subjects of one immutable evidence selection.
//!
//! Call and return field checks ask thousands of questions against the same
//! namespace. Resolve each live subject once, then search exact coordinates.
//! Extent and runtime-selector rows retain the original coverage judgment;
//! structural equality alone cannot authorize an unresolved field or case.
//! This is an invocation-local observation plan, not a declaration roster or
//! cached proof: it cannot recover a retired fact or combine capture epochs.

use facts::{FactContextHandle, FactPlan, PlaceSegment};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use validation::CanonicalPlace;

struct Membership {
    place: CanonicalPlace,
    domain: SymbolHandle,
    instance: SemanticDomainId,
}

pub(crate) struct MembershipEvidence<'checking> {
    program: &'checking dyn validation::CheckingInput,
    // Pin the immutable evidence owner while its prepared subjects are queried.
    _semantic: &'checking FactPlan,
    rows: Vec<Membership>,
    flexible: Vec<usize>,
}

impl<'checking> MembershipEvidence<'checking> {
    pub(crate) fn prepare(
        program: &'checking dyn validation::CheckingInput,
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
                super::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                )
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

    pub(crate) fn contains(
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
                && super::segments_cover_subject(
                    self.program,
                    &row.place.segments,
                    &subject.segments,
                )
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
