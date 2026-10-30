//! Read a finite projection as owned spans separated by current descriptor loads.
//!
//! Geometry requests a load; it never supplies one. Resolve only the next
//! descriptor prefix, then extend that captured supplier by the next owned
//! span. This preserves current overlays without jumping over an unobserved
//! ancestor. Every alternative and every loaded binding reaches the result.

use super::super::stored_origins::{
    CapturedReferenceRead, StorageProjectionCursor, captured_reference_reads,
    completed_reference_projection_reads,
};
use super::*;

pub(super) fn capture(
    program: &dyn crate::CheckingInput,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<CapturedReferenceRead>> {
    // Recast interpretation has its own authenticated geometry route. This
    // entry consumes declaration-relative selectors, not a guessed view.
    if !source.recasts.is_empty() || source.representation.is_some() {
        return None;
    }
    let reference = source_root_type(program, state, index, source.root)?;
    let mut cursor = StorageProjectionCursor::new(program, reference);
    let borrowed_root = program
        .state_parameter_by_symbol(state, source.root)
        .is_some();
    if cursor.reference_access().is_some()
        && (borrowed_root || !crate::stores_reference_shape(program, reference, &[]))
    {
        // A borrowed formal already denotes its ingress storage. A builtin
        // view carries a descriptor rather than an explicit Reference shape.
        cursor.enter_referee()?;
    }
    let mut cuts = Vec::new();
    for position in 0..=source.segments.len() {
        while cursor.reference_access().is_some() {
            cuts.push(position);
            cursor.enter_referee()?;
        }
        if let Some(segment) = source.segments.get(position) {
            if !matches!(
                segment,
                PlaceSegment::Field { .. }
                    | PlaceSegment::Case { .. }
                    | PlaceSegment::FixedIndex { .. }
            ) {
                return None;
            }
            cursor.advance(segment)?;
        }
    }
    cursor.finish()?;
    let initial_end = cuts.first().copied().unwrap_or(source.segments.len());
    let mut initial = source.clone();
    initial.segments.truncate(initial_end);
    let origin = FramePlaceOrigin {
        path: super::super::frame_coordinate_path(program, initial.root, &initial.segments)?,
        precision: FramePathPrecision::Exact,
        source: initial,
    };
    let mut reads = captured_reference_reads(program, &Some(origin), aliases, stored)
        .into_iter()
        .map(|read| read.and_then(admitted))
        .collect::<Option<Vec<_>>>()?;
    if !cuts.is_empty() && reads.iter().any(|read| read.slots.is_empty()) {
        // A spatial answer or lifetime debt did not load this descriptor.
        return None;
    }
    let mut consumed = initial_end;
    for (end, load_required) in cuts
        .into_iter()
        .skip(1)
        .map(|end| (end, true))
        .chain(std::iter::once((source.segments.len(), false)))
    {
        let mut completed = Vec::new();
        for read in reads {
            let before = read.slots.len();
            let mut projected = read.source.origin.clone();
            projected.source = projected
                .source
                .append_segments(&source.segments[consumed..end]);
            projected.path = if projected.source.root.is_valid() {
                super::super::frame_coordinate_path(
                    program,
                    projected.source.root,
                    &projected.source.segments,
                )?
            } else if projected.source.is_static() {
                // Immutable image backing retains its existing footprint;
                // typed selectors still identify the projected value.
                projected.path.clone()
            } else {
                return None;
            };
            for next in completed_reference_projection_reads(program, &read, &projected, stored)? {
                let next = admitted(next)?;
                // A repeated zero-width boundary needs independent evidence;
                // reusing one load cannot authenticate another pointer.
                if load_required && next.slots.len() == before {
                    return None;
                }
                if !completed.contains(&next) {
                    completed.push(next);
                }
            }
        }
        reads = completed;
        consumed = end;
    }
    (!reads.is_empty()).then_some(reads)
}

fn admitted(read: CapturedReferenceRead) -> Option<CapturedReferenceRead> {
    (read.source.lifetime_access.is_none()
        && read.source.origin.precision == FramePathPrecision::Exact
        && read.source.origin.source.output_identity().is_none()
        && read.slots.iter().all(|slot| {
            slot.access.is_readable()
                && slot.place.precision == FramePathPrecision::Exact
                && slot.place.source.root.is_valid()
                && slot.place.source.output_identity().is_none()
        }))
    .then_some(read)
}
