//! Prepared assignment inputs for the flow builder's current semantic sweep.
//!
//! Flow publishes complete contexts and thereafter appends new contexts; writes
//! invalidate selections of context handles, not facts inside an old context.
//! Preparing each selected context once therefore avoids repeating place
//! reconstruction and `self` normalization for every operand read. Queries still
//! traverse their current selection, preserving removals, duplicates, and order.
//!
//! This is private construction scratch, not a cache over arbitrary FactPlans.
//! `FlowBuildContext::discard_output` clears it before the semantic baseline is
//! restored, including when handles would be reused by the next sweep. The typed
//! program also stays immutable throughout that build. Do not expose this reuse
//! to callers that can mutate published facts, places, values, or context refs.

use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::values::{AssignmentObservation, prepare_assignment_observations};
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use facts::{FactContextHandle, FactPayload, FactPlan, ScalarValue};

#[derive(Default)]
struct PreparedContext {
    context: FactContextHandle,
    observations: Vec<AssignmentObservation>,
}

#[derive(Default)]
pub(super) struct AssignmentObservations {
    // Contexts are arena-owned, so their handle already supplies a dense slot.
    // Retain the complete handle as well; a generation mismatch never reuses a
    // former occupant. Empty observations are prepared too, avoiding rescans of
    // the many declaration contexts which carry no assignment values.
    contexts: Vec<PreparedContext>,
}

impl AssignmentObservations {
    pub(super) fn clear(&mut self) {
        self.contexts.clear();
    }

    fn payloads<'a>(
        &'a mut self,
        program: &'a dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &'a [FlowSemanticContextRef],
        subject: &'a CanonicalPlace,
    ) -> impl Iterator<Item = FactPayload> + 'a {
        let literal = crate::values::literal_assignment_payload(program, subject);
        if literal.is_none() {
            for reference in contexts {
                let handle = reference.context;
                let position = handle.arena_index() as usize;
                if !handle.is_valid() {
                    continue;
                }
                if self.contexts.len() <= position {
                    self.contexts
                        .resize_with(position + 1, PreparedContext::default);
                }
                let prepared = &mut self.contexts[position];
                if prepared.context != handle {
                    let context = semantic.contexts.get(handle);
                    prepared.observations =
                        prepare_assignment_observations(program, semantic, context);
                    prepared
                        .observations
                        .sort_by(|left, right| left.place.compare_identity(&right.place));
                    prepared.context = handle;
                }
            }
        }
        let subject_root = subject.root;
        let prepared_contexts = &self.contexts;
        literal
            .into_iter()
            .chain(
                contexts
                    .iter()
                    .filter(move |_| literal.is_none())
                    .flat_map(move |reference| {
                        prepared_contexts
                            .get(reference.context.arena_index() as usize)
                            .filter(|prepared| prepared.context == reference.context)
                            .into_iter()
                            .flat_map(move |prepared| {
                                // Identity ordering groups a root's observations.
                                // Runtime selectors still use semantic equality
                                // below; sorting does not equate their spellings.
                                let root = CanonicalPlace {
                                    root: subject_root,
                                    segments: Vec::new(),
                                };
                                let first = prepared.observations.partition_point(|observation| {
                                    observation.place.compare_identity(&root).is_lt()
                                });
                                let selected = &prepared.observations[first..];
                                let count = selected.partition_point(|observation| {
                                    observation.place.root == subject_root
                                });
                                &selected[..count]
                            })
                            .filter(move |observation| {
                                observation.place.root == subject_root
                                    && crate::flow::canonical_places_agree(
                                        program,
                                        &observation.place,
                                        subject,
                                    )
                            })
                            .map(|observation| observation.payload)
                    }),
            )
    }

    pub(super) fn scalar_value(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FlowSemanticContextRef],
        subject: &CanonicalPlace,
    ) -> Option<ScalarValue> {
        crate::values::scalar_value_from_payloads(
            program,
            semantic,
            self.payloads(program, semantic, contexts, subject),
        )
    }

    fn integer_bounds(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FlowSemanticContextRef],
        subject: &CanonicalPlace,
    ) -> Option<facts::IntegerRange> {
        crate::values::integer_bounds_from_payloads(
            program,
            semantic,
            self.payloads(program, semantic, contexts, subject),
        )
    }
}

impl FlowBuildContext<'_> {
    pub(super) fn scalar_value_at_place(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: HandleSpan<FlowSemanticContextRef>,
        subject: &CanonicalPlace,
    ) -> Option<ScalarValue> {
        self.assignment_observations.scalar_value(
            program,
            semantic,
            self.contexts.semantic_context_refs.span_or_empty(contexts),
            subject,
        )
    }

    pub(super) fn integer_bounds_at_place(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: HandleSpan<FlowSemanticContextRef>,
        subject: &CanonicalPlace,
    ) -> Option<facts::IntegerRange> {
        self.assignment_observations.integer_bounds(
            program,
            semantic,
            self.contexts.semantic_context_refs.span_or_empty(contexts),
            subject,
        )
    }
}
