//! Invocation-local identity batches for the call's existing evidence.
//!
//! Sorting is only a lookup mechanism. First-appearance publication order and
//! each group's original reference order remain unchanged. Distinct fact
//! handles retain their issuers even when their places or payloads match.

use crate::flow::{CallFlowContexts, FlowBuildContext, reference_spans};
use arena::HandleSpan;
use facts::{FactHandle, FactPlan, FactRef, ProgramPoint};
use validation::CanonicalPlace;

pub(super) fn append_place_contexts(
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'_>,
    exit: &mut CallFlowContexts,
    point: ProgramPoint,
    rows: Vec<(CanonicalPlace, FactHandle)>,
) {
    let mut rows: Vec<_> = rows
        .into_iter()
        .enumerate()
        .map(|(order, (place, fact))| (place, fact, order))
        .collect();
    // Stable sorting keeps source order inside an exact-place group. Complete
    // structural identity is intentional: overlapping or equivalent selectors
    // must not silently share an invalidation context.
    rows.sort_by(|(left, _, _), (right, _, _)| left.compare_identity(right));
    let mut rows = rows.into_iter().peekable();
    let mut groups = Vec::new();
    while let Some((place, fact, order)) = rows.next() {
        let mut references: HandleSpan<FactRef> = HandleSpan::empty();
        semantic.append_ref(&mut references, fact);
        while rows.peek().is_some_and(|(next, _, _)| *next == place) {
            let Some((_, fact, _)) = rows.next() else {
                break;
            };
            semantic.append_ref(&mut references, fact);
        }
        groups.push((order, references));
    }
    groups.sort_by_key(|(order, _)| *order);
    for (_, references) in groups {
        reference_spans::append_semantic_context(semantic, build, exit, point, references);
    }
}
