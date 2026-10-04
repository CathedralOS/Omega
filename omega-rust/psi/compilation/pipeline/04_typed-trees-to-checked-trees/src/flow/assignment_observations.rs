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

use crate::flow::{CanonicalPlace, FlowBuildContext, normalized_event_place_root};
use crate::values::{AssignmentObservation, prepare_assignment_observations};
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use facts::{FactContextHandle, FactPayload, FactPlan, ScalarValue};
use typed_trees::TypedTrees;

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
        program: &TypedTrees,
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
                    prepared.context = handle;
                }
            }
        }
        let subject_root = normalized_event_place_root(program, subject.root);
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
                            .flat_map(|prepared| &prepared.observations)
                            .filter(move |observation| {
                                observation.place.root == subject_root
                                    && observation.place.segments == subject.segments
                            })
                            .map(|observation| observation.payload)
                    }),
            )
    }

    fn scalar_value(
        &mut self,
        program: &TypedTrees,
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
        program: &TypedTrees,
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
        program: &TypedTrees,
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
        program: &TypedTrees,
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
