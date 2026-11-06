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
    // The prefix alias owner has already captured this reference value. Its
    // former descriptor home must not be loaded again. Current standalone
    // slot rows shadow older aliases before this query, including unknown
    // replacements; ordinary stored locals still owe their root slot load.
    let captured_root = aliases
        .iter()
        .any(|(name, _)| program.symbols.name(source.root) == name);
    let mut carrier_readable = true;
    if let Some(access) = cursor.reference_access()
        && (usage == ProjectionUse::Address
            || borrowed_root
            || captured_root
            || !crate::stores_reference_shape(program, reference, &[]))
    {
        if usage == ProjectionUse::Contents && !access.is_readable() {
            return None;
        }
        carrier_readable = access.is_readable();
        cursor.enter_referee()?;
    }
    let mut cuts = Vec::new();
    let mut dynamic_address = false;
    for position in 0..=source.segments.len() {
        if usage == ProjectionUse::Address && position == source.segments.len() {
            break;
        }
        while let Some(access) = cursor.reference_access() {
            // An address may select an element of the final owned span.
            // It cannot load an unknown pointer stored inside that element.
            if dynamic_address {
                return None;
            }
            if !carrier_readable || (usage == ProjectionUse::Contents && !access.is_readable()) {
                return None;
            }
            cuts.push(position);
            cursor.enter_referee()?;
            carrier_readable = access.is_readable();
        }
        if let Some(segment) = source.segments.get(position) {
            if usage == ProjectionUse::Address
                && position + 1 == source.segments.len()
                && let PlaceSegment::FixedRange { start, end } = segment
            {
                // This is an address into the existing collection. Earlier
                // pointer loads still populate `cuts` above; the range is not
                // a new owned array or permission to observe its contents.
                cursor.validate_fixed_window(*start, *end)?;
                break;
            }
            if usage == ProjectionUse::Address && matches!(segment, PlaceSegment::Index { .. }) {
                // Formation checks bounds and alignment. Keep the authored
                // selector in its captured address; this supplies no value
                // equality with an element observed at another occurrence.
                dynamic_address = true;
            } else if !matches!(
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
    let reads = captured_reference_reads(program, &Some(origin), aliases, stored)
        .into_iter()
        .map(|read| read.and_then(|read| admitted(program, read, usage)))
        .collect::<Option<Vec<_>>>()?;
    if !cuts.is_empty() && reads.iter().any(|read| read.slots.is_empty()) {
        // A spatial answer or lifetime debt did not load this descriptor.
        return None;
    }
    let spans = cuts
        .into_iter()
        .skip(1)
        .map(|end| (end, true))
        .chain((usage == ProjectionUse::Contents).then_some((source.segments.len(), false)));
    complete_spans(
        program,
        reads,
        &source.segments,
        initial_end,
        spans,
        stored,
        usage,
    )
}

/// The returned field has already supplied this pointer value. Continue its
/// owned suffix without loading that captured pointer from its former slot a
/// second time. Every subsequent pointer still needs a distinct current load.
pub(in crate::machine_calls::calls::write_frames) fn complete_contents(
    program: &dyn crate::CheckingInput,
    source: super::super::stored_calls::SlotSource,
    mut cursor: StorageProjectionCursor<'_>,
    path: &[PlaceSegment],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<CapturedReferenceRead>> {
    let mut cuts = Vec::new();
    for position in 0..=path.len() {
        while let Some(access) = cursor.reference_access() {
            if !access.is_readable() {
                return None;
            }
            cuts.push((position, true));
            cursor.enter_referee()?;
        }
        if let Some(segment) = path.get(position) {
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
    let read = admitted(
        program,
        CapturedReferenceRead {
            source,
            slots: Vec::new(),
        },
        ProjectionUse::Contents,
    )?;
    cuts.push((path.len(), false));
    complete_spans(
        program,
        vec![read],
        path,
        0,
        cuts,
        stored,
        ProjectionUse::Contents,
    )
}

/// Retain the physical dependencies of an already captured scalar. Geometry
/// still validates the whole suffix, but an unknown element in its final owned
/// span depends on the containing collection. No coarse footprint may stand in
/// for a descriptor load, and this result never exports an exact read source.
pub(in crate::machine_calls::calls::write_frames) fn contents_invalidation_paths(
    program: &dyn crate::CheckingInput,
    source: super::super::stored_calls::SlotSource,
    mut cursor: StorageProjectionCursor<'_>,
    path: &[PlaceSegment],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<String>> {
    let mut spans = Vec::new();
    let mut first_dynamic = None;
    for position in 0..=path.len() {
        while let Some(access) = cursor.reference_access() {
            if !access.is_readable() || first_dynamic.is_some() {
                // The selector's old AST cannot identify this pointer slot.
                // A captured exact selector arrives as FixedIndex instead.
                return None;
            }
            spans.push((position, true));
            cursor.enter_referee()?;
        }
        if let Some(segment) = path.get(position) {
            cursor.advance(segment)?;
            if matches!(segment, PlaceSegment::Index { .. }) {
                first_dynamic.get_or_insert(position);
            }
        }
    }
    cursor.finish()?;
    if source.origin.precision == FramePathPrecision::CollectionCoarse {
        // A completed window already names a physical containing region. It
        // can retire an existing scalar observation anywhere in that region,
        // but cannot identify a descriptor which the suffix would load.
        if !spans.is_empty()
            || source.lifetime_access.is_some()
            || source.origin.source.output_identity().is_some()
            || !source.origin.source.builtin_coordinates
            || (!source.origin.source.root.is_valid() && !source.origin.source.is_static())
        {
            return None;
        }
        if crate::trace_flags::struct_trace() {
            eprintln!(
                "STRUCT captured-reference-coarse-dependency path={} source={:?}",
                source.origin.path, source.origin.source
            );
        }
        return Some(vec![source.origin.path]);
    }
    let read = admitted(
        program,
        CapturedReferenceRead {
            source,
            slots: Vec::new(),
        },
        ProjectionUse::Contents,
    )?;
    // All required pointer loads precede the first unknown selector. Finish
    // only that physical prefix; its existing frame spelling conservatively
    // covers every element and descendant in the remaining owned suffix.
    spans.push((first_dynamic.unwrap_or(path.len()), false));
    let reads = complete_spans(
        program,
        vec![read],
        path,
        0,
        spans,
        stored,
        ProjectionUse::Contents,
    )?;
    let backing = super::read_backing::from_reads(program, reads, Vec::new())?;
    let mut paths = backing.values;
    for binding in backing.bindings {
        if !paths.contains(&binding.path) {
            paths.push(binding.path);
        }
    }
    Some(paths)
}

fn complete_spans(
    program: &dyn crate::CheckingInput,
    mut reads: Vec<CapturedReferenceRead>,
    path: &[PlaceSegment],
    mut consumed: usize,
    spans: impl IntoIterator<Item = (usize, bool)>,
    stored: &[StoredLocalOrigins],
    usage: ProjectionUse,
) -> Option<Vec<CapturedReferenceRead>> {
    for (end, load_required) in spans {
        let mut completed = Vec::new();
        for read in reads {
            let before = read.slots.len();
            let mut projected = read.source.origin.clone();
            projected.source = projected.source.append_segments(&path[consumed..end]);
            projected.path = if projected.source.root.is_valid() {
                super::super::frame_coordinate_path(
                    program,
                    projected.source.root,
                    &projected.source.segments,
                )?
            } else if projected.source.is_static() || projected.source.result_home().is_some() {
                // Image backing and opaque result referents retain their
                // existing footprint; typed selectors still identify the
                // projected value.
                projected.path.clone()
            } else {
                return None;
            };
            let next_reads =
                completed_reference_projection_reads(program, &read, &projected, stored);
            let Some(next_reads) = next_reads else {
                if crate::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT read-projection-refused phase=next-descriptor-suppliers source={:?} suffix={:?} loaded={before}",
                        read.source,
                        &path[consumed..end]
                    );
                }
                return None;
            };
            for next in next_reads {
                let next = admitted(program, next, usage)?;
                // A repeated zero-width boundary needs independent evidence;
                // reusing one load cannot authenticate another pointer.
                if load_required && next.slots.len() == before {
                    if crate::trace_flags::struct_trace() {
                        eprintln!(
                            "STRUCT read-projection-refused phase=missing-independent-descriptor-load source={:?} suffix={:?} loaded={before} next={next:?}",
                            read.source,
                            &path[consumed..end]
                        );
                    }
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

fn admitted(
    program: &dyn crate::CheckingInput,
    read: CapturedReferenceRead,
    usage: ProjectionUse,
) -> Option<CapturedReferenceRead> {
    let reason = if read.source.lifetime_access.is_some() {
        Some("lifetime debt is not a descriptor load")
    } else if read.source.origin.source.output_identity().is_some() {
        Some("opaque output has no independently loaded contents")
    } else if !resolved_read_coordinate(program, &read.source.origin) {
        Some("supplier coordinate is not resolved")
    } else if read
        .slots
        .iter()
        .any(|slot| usage != ProjectionUse::Address && !slot.access.is_readable())
    {
        Some("descriptor slot is not readable")
    } else if read.slots.iter().any(|slot| {
        !slot.place.source.root.is_valid() || slot.place.source.output_identity().is_some()
    }) {
        Some("descriptor slot has no declaration namespace")
    } else if read
        .slots
        .iter()
        .any(|slot| !resolved_read_coordinate(program, &slot.place))
    {
        Some("descriptor slot coordinate is not resolved")
    } else {
        None
    };
    if let Some(reason) = reason {
        if crate::trace_flags::struct_trace() {
            eprintln!("STRUCT read-admission-refused reason={reason} read={read:?}");
        }
        None
    } else {
        Some(read)
    }
}

/// Load records already own their access, lifetime and predecessor custody.
/// This authenticates only their retained declaration coordinates: the write
/// footprint remains coarse and supplies no equality with one selected slot.
/// Unknown selectors, ranges and unresolved applications cannot refine it.
pub(in crate::machine_calls::calls::write_frames) fn resolved_read_coordinate(
    program: &dyn crate::CheckingInput,
    origin: &FramePlaceOrigin,
) -> bool {
    if origin.precision == FramePathPrecision::Exact {
        return true;
    }
    let source = &origin.source;
    if !source.root.is_valid()
        || !source.builtin_coordinates
        || source.output_identity().is_some()
        || !source.recasts.is_empty()
        || source.representation.is_some()
    {
        if crate::trace_flags::struct_trace() {
            eprintln!(
                "STRUCT read-coordinate-refused phase=namespace-builtin-or-representation origin={origin:?}"
            );
        }
        return false;
    }
    let Some(reference) = crate::declaration_type_reference(program, source.root) else {
        if crate::trace_flags::struct_trace() {
            eprintln!(
                "STRUCT read-coordinate-refused phase=source-declaration-type origin={origin:?}"
            );
        }
        return false;
    };
    let mut cursor = StorageProjectionCursor::new(program, reference);
    for segment in &source.segments {
        while cursor.reference_access().is_some() {
            if cursor.enter_referee().is_none() {
                if crate::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT read-coordinate-refused phase=reference-or-case-crossing origin={origin:?} segment={segment:?}"
                    );
                }
                return false;
            }
        }
        if cursor.advance_fixed(segment).is_none() {
            if crate::trace_flags::struct_trace() {
                eprintln!(
                    "STRUCT read-coordinate-refused phase=declared-field-case-or-fixed-extent origin={origin:?} segment={segment:?}"
                );
            }
            return false;
        }
    }
    cursor.finish().is_some()
}
