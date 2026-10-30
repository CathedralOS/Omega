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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectionUse {
    Address,
    Contents,
}

pub(super) fn capture(
    program: &dyn crate::CheckingInput,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<CapturedReferenceRead>> {
    capture_projection(
        program,
        state,
        index,
        source,
        aliases,
        stored,
        ProjectionUse::Contents,
    )
}

/// A lender can form an address through write-only storage. Only an actual
/// stored-pointer crossing reads a descriptor; selecting the final place does
/// not read its contents or the pointer value stored there.
pub(super) fn validate_address(
    program: &dyn crate::CheckingInput,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    stored: &[StoredLocalOrigins],
) -> Option<()> {
    capture_projection(
        program,
        state,
        index,
        source,
        &[],
        stored,
        ProjectionUse::Address,
    )
    .map(|_| ())
}

fn capture_projection(
    program: &dyn crate::CheckingInput,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    usage: ProjectionUse,
) -> Option<Vec<CapturedReferenceRead>> {
    // Recast interpretation has its own authenticated geometry route.
    if !source.recasts.is_empty() || source.representation.is_some() {
        return None;
    }
    let reference = source_root_type(program, state, index, source.root)?;
    let mut cursor = StorageProjectionCursor::new(program, reference);
    let borrowed_root = program
        .state_parameter_by_symbol(state, source.root)
        .is_some();
    let mut carrier_readable = true;
    if let Some(access) = cursor.reference_access()
        && (usage == ProjectionUse::Address
            || borrowed_root
            || !crate::stores_reference_shape(program, reference, &[]))
    {
        if usage == ProjectionUse::Contents && !access.is_readable() {
            return None;
        }
        carrier_readable = access.is_readable();
        cursor.enter_referee()?;
    }
    let mut cuts = Vec::new();
    for position in 0..=source.segments.len() {
        if usage == ProjectionUse::Address && position == source.segments.len() {
            break;
        }
        while let Some(access) = cursor.reference_access() {
            if !carrier_readable || (usage == ProjectionUse::Contents && !access.is_readable()) {
                return None;
            }
            cuts.push(position);
            cursor.enter_referee()?;
            carrier_readable = access.is_readable();
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
    if usage == ProjectionUse::Address && cuts.is_empty() {
        return Some(Vec::new());
    }
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
        .map(|read| read.and_then(|read| admitted(read, usage)))
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
        .chain((usage == ProjectionUse::Contents).then_some((source.segments.len(), false)))
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
                let next = admitted(next, usage)?;
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

fn admitted(read: CapturedReferenceRead, usage: ProjectionUse) -> Option<CapturedReferenceRead> {
    (read.source.lifetime_access.is_none()
        && read.source.origin.precision == FramePathPrecision::Exact
        && read.source.origin.source.output_identity().is_none()
        && read.slots.iter().all(|slot| {
            (usage == ProjectionUse::Address || slot.access.is_readable())
                && slot.place.precision == FramePathPrecision::Exact
                && slot.place.source.root.is_valid()
                && slot.place.source.output_identity().is_none()
        }))
    .then_some(read)
}
