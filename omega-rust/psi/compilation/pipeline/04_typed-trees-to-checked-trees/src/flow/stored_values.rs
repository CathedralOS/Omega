//! Associate resident storage with its exact completed producer. This is a
//! value association, not permission to copy, retain a loan, or grant custody.
//! Consumers select the write relation they need: descriptor identity survives
//! descendant writes, while correspondence of contents does not. Missing rows
//! and unknown writes never fall back to a historical initializer.

use arena::{Handle, HandleSpan};
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowOperandReferent};
use validation::CanonicalPlace;

pub(crate) use validation::completed_storage::{StoredObservation, StoredOperand, at_snapshot};

pub(crate) fn append_writes(
    control: &mut FlowControlFacts,
    previous: Handle<checked_trees::FlowStorageSnapshot>,
    bindings: HandleSpan<FlowOperandReferent>,
    contents: HandleSpan<FlowOperandReferent>,
    bindings_complete: bool,
    contents_complete: bool,
) -> Handle<checked_trees::FlowStorageSnapshot> {
    if bindings.is_empty() && contents.is_empty() && bindings_complete && contents_complete {
        return previous;
    }
    control
        .storage_snapshots
        .insert(checked_trees::FlowStorageSnapshot {
            previous,
            operation: checked_trees::FlowStorageOperation::Writes {
                bindings,
                contents,
                bindings_complete,
                contents_complete,
            },
        })
}

pub(crate) fn append_place_writes(
    control: &mut FlowControlFacts,
    previous: Handle<checked_trees::FlowStorageSnapshot>,
    bindings: Option<&[CanonicalPlace]>,
    contents: Option<&[CanonicalPlace]>,
) -> Handle<checked_trees::FlowStorageSnapshot> {
    fn rows(
        control: &mut FlowControlFacts,
        places: Option<&[CanonicalPlace]>,
    ) -> HandleSpan<FlowOperandReferent> {
        let rows = places
            .into_iter()
            .flatten()
            .map(|place| {
                let segments = control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied());
                FlowOperandReferent {
                    root: place.root,
                    segments,
                }
            })
            .collect::<Vec<_>>();
        control.operand_origin_writes.insert_many(rows)
    }
    let binding_rows = rows(control, bindings);
    let content_rows = rows(control, contents);
    append_writes(
        control,
        previous,
        binding_rows,
        content_rows,
        bindings.is_some(),
        contents.is_some(),
    )
}

pub(crate) fn append_store(
    control: &mut FlowControlFacts,
    previous: Handle<checked_trees::FlowStorageSnapshot>,
    destination: &CanonicalPlace,
    value: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
) -> Handle<checked_trees::FlowStorageSnapshot> {
    if !destination
        .segments
        .iter()
        .all(super::place_segment_has_static_identity)
    {
        // The preceding may-write already retired overlapping cells. An
        // unresolved selector cannot bind a later, re-read selector to this RHS.
        return previous;
    }
    let segments = control
        .operand_referent_segments
        .insert_many(destination.segments.iter().copied());
    let projection = control
        .operand_retained_paths
        .insert_many(projection.iter().copied());
    control
        .storage_snapshots
        .insert(checked_trees::FlowStorageSnapshot {
            previous,
            operation: checked_trees::FlowStorageOperation::Store {
                destination: FlowOperandReferent {
                    root: destination.root,
                    segments,
                },
                value,
                projection,
            },
        })
}

pub(crate) fn join(
    control: &mut FlowControlFacts,
    predecessors: &[Handle<checked_trees::FlowStorageSnapshot>],
) -> Handle<checked_trees::FlowStorageSnapshot> {
    if let Some(first) = predecessors.first()
        && predecessors.iter().all(|value| value == first)
    {
        return *first;
    }
    let predecessors = control
        .storage_predecessors
        .insert_many(predecessors.iter().copied());
    control
        .storage_snapshots
        .insert(checked_trees::FlowStorageSnapshot {
            previous: Handle::invalid(),
            operation: checked_trees::FlowStorageOperation::Join { predecessors },
        })
}
