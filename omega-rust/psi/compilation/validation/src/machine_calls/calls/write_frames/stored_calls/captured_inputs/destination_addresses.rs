//! An assignment captures storage before its RHS can replace a descriptor.
//! Walk its canonical projection spine once. The last pointer receiver supplies
//! a completed descriptor; the remaining span stays entirely in owned storage.
//! Current reference slots are a separate result of operand execution.

use super::*;
use crate::machine_calls::calls::write_frames::{FramePathPrecision, FramePlaceOrigin};
use std::collections::BTreeMap;

type CaptureKey = (u32, u32);

pub(in crate::machine_calls::calls::write_frames::stored_calls) fn assignment_addresses(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    target: &SlotArgumentCapture,
    receiving: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
) -> Result<Vec<FramePlaceOrigin>, &'static str> {
    let query = Inputs {
        program,
        machine,
        captured: target,
        symbols,
        entry: &target.context,
    };
    // Actual captures are sparse global handles. Duplicate observations cannot
    // select an address, even if both observations have the same possible roots.
    let mut captures: BTreeMap<CaptureKey, Option<&SlotArgumentCapture>> = BTreeMap::new();
    let mut pending = vec![target];
    while let Some(captured) = pending.pop() {
        captures
            .entry(key(captured.expression))
            .and_modify(|previous| *previous = None)
            .or_insert(Some(captured));
        pending.extend(captured.operands.iter());
    }
    let capture = |expression| {
        captures
            .get(&key(expression))
            .copied()
            .flatten()
            .ok_or("destination has no unique completed operand capture")
    };
    let (state, _, _) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(target.expression),
    )
    .ok_or("destination has no exact statement owner")?;
    crate::value_has_builtin_coordinates(program, machine, state, target.expression)
        .then_some(())
        .ok_or("destination projection has no selected builtin coordinates")?;
    let mut place = crate::canonical_place_from_expression(program, target.expression)
        .ok_or("destination has no canonical storage projection")?;
    // The ordinary receiving-shape owner also resolves omitted endpoints from
    // the fixed collection declaration. Keep earlier selectors captured; only
    // this final window gets the same finite bounds used by assignment typing.
    if let Some(shape) = crate::array_window_shape(
        program,
        machine,
        state,
        target.expression,
        &mut crate::ImmutableBoundLookup::new(program),
    ) {
        *place
            .segments
            .last_mut()
            .ok_or("destination window has no canonical collection selector")? =
            facts::PlaceSegment::FixedRange {
                start: shape.start.ok_or("destination window has no fixed start")?,
                end: shape.end.ok_or("destination window has no fixed end")?,
            };
    }
    let mut end = place.segments.len();
    let mut expression = target.expression;
    let mut spine = Vec::new();
    while let Some(receiver) = query.projection_receiver(expression) {
        capture(expression)?;
        capture(receiver)?;
        let last = end
            .checked_sub(1)
            .ok_or("destination projection has no canonical suffix")?;
        let start = match place.segments[last] {
            facts::PlaceSegment::Field { symbol } => {
                if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                    let start = last
                        .checked_sub(1)
                        .ok_or("destination field lost its case selector")?;
                    if place.segments[start] != (facts::PlaceSegment::Case { variant }) {
                        return Err("destination field changes its canonical case selector");
                    }
                    start
                } else {
                    last
                }
            }
            facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::Index { .. }
            | facts::PlaceSegment::FixedRange { .. } => last,
            _ => return Err("destination has an unsupported canonical projection"),
        };
        spine.push((receiver, start, end));
        end = start;
        expression = receiver;
    }
    let base = capture(expression)?;
    let base_place = crate::canonical_place_from_expression(program, expression)
        .ok_or("destination base has no canonical storage place")?;
    if base_place.root != place.root || base_place.segments != place.segments[..end] {
        return Err("destination spine does not retain its canonical base");
    }
    let base_type = crate::expression_result_type_reference(program, machine, state, expression)
        .ok_or("destination base has no checked result type")?;
    let mut cursor = stored_origins::StorageProjectionCursor::new(program, base_type);
    let mut descriptor = None;
    for (receiver, start, end) in spine.into_iter().rev() {
        if cursor.reference_access().is_some() {
            descriptor = Some((capture(receiver)?, start));
            cursor
                .enter_referee()
                .ok_or("destination receiver has no reference backing")?;
        }
        advance_destination_span(
            &mut cursor,
            &place.segments[start..end],
            end == place.segments.len(),
        )?;
    }
    let window = match place.segments.last() {
        Some(facts::PlaceSegment::FixedRange { start, end }) => Some((*start, *end)),
        _ => None,
    };
    // The prepared receiving array below owns a window's type; its range
    // expression need not produce an independently typed value.
    if window.is_none() {
        let actual =
            crate::expression_result_type_reference(program, machine, state, target.expression)
                .ok_or("destination has no checked result type")?;
        if program.normalized_type_identity(actual) != program.normalized_type_identity(receiving) {
            cursor
                .reference_access()
                .ok_or("destination result is not a reference to its receiving cell")?;
            descriptor = Some((target, place.segments.len()));
            cursor
                .enter_referee()
                .ok_or("destination result has no reference backing")?;
        }
    }
    if !matches_receiving(cursor, window, receiving) {
        return Err("destination projection changes its receiving storage type");
    }
    let (mut origins, start) = if let Some((captured, start)) = descriptor {
        let reference =
            crate::expression_result_type_reference(program, machine, state, captured.expression)
                .ok_or("destination descriptor has no checked result type")?;
        let shell = crate::reference_shell(program, reference)
            .ok_or("destination receiver has no checked reference type")?;
        // Revalidate this complete owned span in the selected descriptor's
        // applied type, retaining generic substitutions and every case/index.
        let mut owned = stored_origins::StorageProjectionCursor::new(program, shell.referee);
        advance_destination_span(&mut owned, &place.segments[start..], true)?;
        if !matches_receiving(owned, window, receiving) {
            return Err("captured descriptor suffix changes the receiving type");
        }
        let (sources, _) = partition_sources(
            query
                .reference(
                    captured.expression,
                    reference,
                    false,
                    &mut captured.inference.clone(),
                )
                .ok_or("destination descriptor has no complete captured suppliers")?,
        );
        let mut origins = Vec::new();
        for source in sources {
            let origin = match source {
                ReferenceSource::Captured(observed)
                    if observed.source.lifetime_access.is_none() =>
                {
                    observed.source.origin
                }
                ReferenceSource::Symbolic(origin)
                    if origin.source.is_static()
                        || origin.source.output_identity().is_some()
                        || origin.source.result_home().is_some() =>
                {
                    origin
                }
                _ => return Err("destination descriptor retains an uncaptured spatial supplier"),
            };
            require_namespace(program, &origin)?;
            origins.push(origin);
        }
        if origins.is_empty() {
            return Err("destination descriptor has no spatial backing");
        }
        (origins, start)
    } else {
        // This selects the base cell without reading the value stored there.
        // The ordinary storage owner authenticates any ancestor pointer loads
        // against this base's own completed cut, never the later RHS inventory.
        let source = super::super::super::frame_place_path(program, expression)
            .ok_or("destination base has no canonical storage source")?;
        let origins = stored_origins::canonical_storage_origins(
            program,
            &source,
            &base.context.aliases,
            &base.context.divergent,
            &base.context.stored,
        )
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("destination base has no complete captured storage sources")?;
        if origins.is_empty() {
            return Err("destination base has no captured storage");
        }
        (origins, end)
    };
    for origin in &mut origins {
        require_namespace(program, origin)?;
        origin.source = origin.source.append_segments(&place.segments[start..]);
        require_namespace(program, origin)?;
        if origin.precision == FramePathPrecision::Exact {
            let (path, precision) = super::super::super::place_paths::coordinate_storage_path(
                program,
                &origin.path,
                &place.segments[start..],
            );
            origin.path = path;
            origin.precision = precision;
        }
    }
    Ok(origins)
}

fn advance_destination_span(
    cursor: &mut stored_origins::StorageProjectionCursor<'_>,
    segments: &[facts::PlaceSegment],
    completes_destination: bool,
) -> Result<(), &'static str> {
    for (ordinal, segment) in segments.iter().enumerate() {
        if completes_destination
            && ordinal + 1 == segments.len()
            && let facts::PlaceSegment::FixedRange { start, end } = segment
        {
            cursor
                .validate_fixed_window(*start, *end)
                .ok_or("destination window is outside its captured collection")?;
        } else {
            cursor.advance(segment).ok_or(
                "destination projection crosses an uncaptured descriptor or unsupported coordinate",
            )?;
        }
    }
    Ok(())
}

fn matches_receiving(
    cursor: stored_origins::StorageProjectionCursor<'_>,
    window: Option<(usize, usize)>,
    receiving: TypeReferenceHandle,
) -> bool {
    if let Some((start, end)) = window {
        cursor.has_declared_window_type(start, end, receiving)
    } else {
        cursor.has_declared_type(receiving)
    }
}

fn key(expression: ExpressionHandle) -> CaptureKey {
    (expression.arena_index(), expression.generation())
}

fn require_namespace(
    program: &dyn crate::CheckingInput,
    origin: &FramePlaceOrigin,
) -> Result<(), &'static str> {
    if !origin.source.builtin_coordinates {
        return Err("destination has no checked primitive coordinates");
    }
    if !(origin.source.is_static()
        || origin.source.output_identity().is_some()
        || origin.source.result_home().is_some())
    {
        return if origin.source.root.is_valid() {
            Ok(())
        } else {
            Err("destination has no authenticated storage namespace")
        };
    }
    // Terminal namespaces are already completed origins, not declarations to
    // guess or descriptor homes to reload. This owner validates output types
    // and projections while retaining the exact producing call identity.
    let sources =
        stored_origins::canonical_reference_sources(program, &Some(origin.clone()), &[], &[])
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or("destination invocation namespace has no checked type or projection")?;
    if sources.is_empty()
        || sources
            .iter()
            .any(|source| source.lifetime_access.is_some() || source.origin != *origin)
    {
        return Err("destination invocation namespace changes its captured source");
    }
    Ok(())
}
