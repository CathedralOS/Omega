//! Instantiate normal-exit relations against completed caller operands.
//! Owned descriptor words use their operand cuts; a borrowed address subsequently
//! loads storage from the invocation cut. Intermediate constructor cuts conservatively
//! retain possible backing loans without claiming exact scalar predicates.

use super::super as frames;
use super::*;

pub(super) fn instantiate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    completed: &CompletedArguments,
    relative: &[SlotEffect],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let arguments = &completed.expressions;
    if parameters.len() != arguments.len()
        || completed.contexts.len() != arguments.len()
        || !context.divergent.is_empty()
    {
        return None;
    }
    let substitute = |root, segments: &[PlaceSegment], inference: &mut FrameInference| {
        let index = typed_trees::signature::state_parameter_position(parameters, root)?;
        destination_source(
            program,
            machine,
            &parameters[index],
            *arguments.get(index)?,
            segments,
            &completed.contexts[index],
            context,
            symbols,
            inference,
        )
    };
    let mut output = Vec::new();
    for effect in relative {
        // An identity row proves preservation; it has no store to instantiate.
        // In particular, a wildcard default is not an executable selector.
        if effect.incoming && effect.sources.as_ref()?.is_empty() {
            continue;
        }
        let destination = substitute(effect.root, &effect.segments, inference)?;
        let mut sources = Vec::new();
        for supplied in effect.sources.as_ref()? {
            let source = &supplied.origin;
            if !source.source.is_static() {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.source.root,
                )?;
                if !frames::type_reference_is_reference(program, parameters[index].type_reference) {
                    for captured in &completed.histories[index] {
                        for source in aggregate_argument_sources(
                            program,
                            machine,
                            &parameters[index],
                            *arguments.get(index)?,
                            supplied,
                            captured,
                            symbols,
                            inference,
                        )? {
                            if !sources.contains(&source) {
                                sources.push(source);
                            }
                        }
                    }
                    continue;
                }
            }
            // Keep the actual descriptor home beside its backing. Resolving
            // only the backing would turn an exclusive transfer into a fresh
            // competing loan of the same referent.
            let carrier = if supplied.carrier.is_static() {
                supplied.carrier.clone()
            } else {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    supplied.carrier.root,
                )?;
                let actual = *arguments.get(index)?;
                let (state, _, boundary_index) = frames::caller_aliases::caller_statement_owner(
                    program,
                    machine,
                    frames::caller_aliases::CallerWriteSite::Expression(actual),
                )?;
                // A source descriptor is an evaluated reference value. Its
                // producer relation names its carrier, including image storage
                // with no declaration home. Keep this separate from resolving
                // its backing below: an exclusive descriptor's supplying home
                // is part of the transferred permission, not its referent.
                let relation = frames::result_origins::reference_result_relation(
                    program,
                    machine,
                    state,
                    boundary_index,
                    actual,
                    parameters[index].type_reference,
                    symbols,
                    inference,
                )?;
                let origins = relation
                    .references
                    .into_iter()
                    .map(|leaf| leaf.origin)
                    .collect::<Option<Vec<_>>>()?;
                let actual = frames::place_paths::single_place_origin(origins)?.source;
                if (!actual.root.is_valid() && !actual.is_static()) || !actual.builtin_coordinates {
                    return None;
                }
                actual.append_relative(program, &supplied.carrier)
            };
            let candidates = if source.source.is_static() {
                vec![Some(supplied.clone())]
            } else {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.source.root,
                )?;
                let actual = *arguments.get(index)?;
                let mut candidates = Vec::new();
                for capture in &completed.histories[index] {
                    let captured = argument_origin(
                        program,
                        machine,
                        &parameters[index],
                        actual,
                        capture,
                        symbols,
                        inference,
                    )?;
                    let mut projected = captured.clone();
                    projected.source = captured.source.append_relative(program, &source.source);
                    if source.precision == FramePathPrecision::CollectionCoarse {
                        projected.precision = FramePathPrecision::CollectionCoarse;
                    }
                    projected.path = if projected.source.is_static() {
                        String::new()
                    } else {
                        path(program, &projected.source)
                    };
                    let projected_sources = stored_origins::captured_reference_projection_sources(
                        program,
                        &captured,
                        &projected,
                        &context.stored,
                    );
                    for source in projected_sources {
                        if !candidates.contains(&source) {
                            candidates.push(source);
                        }
                    }
                }
                candidates
            };
            for candidate in candidates {
                let candidate = candidate?;
                let source = &candidate.origin.source;
                if (!source.root.is_valid() && !source.is_static()) || !source.builtin_coordinates {
                    return None;
                }
                let source = SlotSource {
                    origin: candidate.origin,
                    carrier: if candidate.lifetime_access.is_some() {
                        candidate.carrier
                    } else {
                        carrier.clone()
                    },
                    lifetime_access: candidate.lifetime_access.or(supplied.lifetime_access),
                };
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
        output.push(SlotEffect {
            root: destination.root,
            segments: destination.segments,
            sources: Some(sources),
            incoming: effect.incoming,
            may_select: effect.may_select,
            kind: effect.kind,
        });
    }
    Some(output)
}

/// Resolve each pointer boundary before appending the destination slot.
/// Only the selected descriptor is interpreted; the final slot's old contents
/// never become its own storage address.
#[allow(clippy::too_many_arguments)]
pub(super) fn destination_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    mut segments: &[PlaceSegment],
    captured: &CollectedStatementPrefix,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FrameSourcePlace> {
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
        ) && !matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid())
    }) {
        return None;
    }
    let mut reference = parameter.type_reference;
    let mut base = None;
    if let Some(shell) = crate::reference_shell(program, reference) {
        if !shell.access.is_exclusive() {
            return None;
        }
        base = Some(argument_origin(
            program, machine, parameter, actual, captured, symbols, inference,
        )?);
        reference = shell.referee;
    }
    loop {
        let inventory = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            reference,
            true,
        )?;
        let mut boundaries = inventory.references.iter().filter(|leaf| {
            leaf.local_segments.len() < segments.len() && segments.starts_with(&leaf.local_segments)
        });
        let Some(boundary) = boundaries.next() else {
            let leaf = stored_origins::projected_storage_type(program, reference, segments)?;
            crate::reference_shell(program, leaf)?;
            let base = base?;
            if !base.source.root.is_valid()
                || base.source.is_static()
                || !base.source.builtin_coordinates
            {
                return None;
            }
            return Some(base.source.append_segments(segments));
        };
        if boundaries.next().is_some() {
            return None;
        }
        let shell = crate::reference_shell(
            program,
            stored_origins::projected_storage_type(program, reference, &boundary.local_segments)?,
        )?;
        if !shell.access.is_exclusive() {
            return None;
        }
        let candidates = if let Some(captured) = &base {
            let mut projected = captured.clone();
            projected.source = captured.source.append_segments(&boundary.local_segments);
            projected.path = path(program, &projected.source);
            stored_origins::captured_reference_projection_origins(
                program,
                captured,
                &projected,
                &context.stored,
            )
            .into_iter()
            .collect::<Option<Vec<_>>>()?
        } else {
            let relative = FrameSourcePlace {
                root: parameter.symbol,
                segments: boundary.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            };
            aggregate_argument_sources(
                program,
                machine,
                parameter,
                actual,
                &SlotSource {
                    lifetime_access: None,
                    origin: FramePlaceOrigin {
                        path: path(program, &relative),
                        precision: FramePathPrecision::Exact,
                        source: relative.clone(),
                    },
                    carrier: relative,
                },
                captured,
                symbols,
                inference,
            )?
            .into_iter()
            .map(|source| source.origin)
            .collect()
        };
        let mut candidates = candidates.into_iter();
        base = Some(candidates.next()?);
        if candidates.next().is_some() {
            return None;
        }
        reference = shell.referee;
        segments = &segments[boundary.local_segments.len()..];
    }
}

/// Instantiate value operands through their returned leaf relation before
/// resolving caller backing. The symbolic leaf is the supplying carrier;
/// resolving it first would lose the exclusive permission being transferred.
/// This is shared by names, projections, constructors and computed results.
#[allow(clippy::too_many_arguments)]
pub(super) fn aggregate_argument_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    supplied: &SlotSource,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotSource>> {
    // A relative normal-exit row must still identify the same input leaf for
    // carrier transport and backing substitution. Do not pair independent
    // alternatives merely because their referents happen to overlap.
    if supplied.carrier != supplied.origin.source {
        return None;
    }
    let carriers = frames::result_origins::instantiate_source(
        program,
        machine,
        parameter,
        actual,
        &supplied.origin,
        symbols,
        inference,
        true,
        &|expression, reference, _, inference| {
            let (state, _, index) = frames::caller_aliases::caller_statement_owner(
                program,
                machine,
                frames::caller_aliases::CallerWriteSite::Expression(expression),
            )?;
            frames::result_origins::reference_result_relation(
                program, machine, state, index, expression, reference, symbols, inference,
            )?
            .references
            .into_iter()
            .map(|leaf| leaf.origin)
            .collect()
        },
        &|expression, reference, _| {
            let (state, before, _) = frames::caller_aliases::caller_statement_owner(
                program,
                machine,
                frames::caller_aliases::CallerWriteSite::Expression(expression),
            )?;
            stored_origins::reference_leaves_before_statement_for_query(
                program, state, before, expression, reference,
                // This query supplies descriptor homes. Resolving the current
                // contents here would replace the actual carrier with its
                // pointee and lose the permission transferred by the call.
                // The canonical_reference_sources join below resolves that
                // symbolic home against this operand's captured prefix once.
                None, None, true,
            )
        },
    )?;
    let mut sources = Vec::new();
    for carrier in carriers {
        if (!carrier.source.root.is_valid() && !carrier.source.is_static())
            || !carrier.source.builtin_coordinates
        {
            return None;
        }
        for candidate in stored_origins::canonical_reference_sources(
            program,
            &Some(carrier.clone()),
            &context.aliases,
            &context.stored,
        ) {
            let candidate = candidate?;
            let origin = candidate.origin;
            if (!origin.source.root.is_valid() && !origin.source.is_static())
                || !origin.source.builtin_coordinates
            {
                return None;
            }
            let source = SlotSource {
                origin,
                carrier: if candidate.lifetime_access.is_some() {
                    candidate.carrier
                } else {
                    carrier.source.clone()
                },
                lifetime_access: candidate.lifetime_access.or(supplied.lifetime_access),
            };
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
    }
    (!sources.is_empty()).then_some(sources)
}

pub(super) fn argument_origin(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    if frames::type_reference_is_reference(program, parameter.type_reference) {
        return frames::reference_subjects::backing_origin(
            program,
            machine,
            actual,
            symbols,
            inference,
            &context.aliases,
            &context.stored,
        );
    }
    let source = stored_origins::binding_source(program, actual, &context.aliases)?;
    if !source.root.is_valid() || !source.builtin_coordinates {
        return None;
    }
    let (path, precision) = frames::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    );
    Some(FramePlaceOrigin {
        path,
        precision,
        source,
    })
}
