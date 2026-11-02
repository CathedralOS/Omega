//! Instantiate normal-exit relations against exact completed caller observations.
//! Completed operands retain their exact source cuts and carrier/backing association.

use super::*;

pub(super) struct InstantiatedSlots {
    pub(super) effects: Vec<SlotEffect>,
    /// Exact input prefixes discovered while instantiating demanded effects.
    /// Keeping them beside the result avoids querying an argument a second
    /// time or counting a callee-authored suffix as argument growth.
    pub(super) substitutions: Vec<(FrameSourcePlace, FrameSourcePlace)>,
}

pub(super) fn instantiate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    relative: &[SlotEffect],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<InstantiatedSlots> {
    if parameters.len() != arguments.len() {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE stored-reference-input-context-refusal machine={:?} phase={} parameter_count={} capture_count={} divergent_count={}",
                machine.symbol,
                if parameters.len() != arguments.len() {
                    "parameter-count"
                } else {
                    "divergent-entry"
                },
                parameters.len(),
                arguments.len(),
                context.divergent.len(),
            );
        }
        return None;
    }
    // Every demanded actual uses its own completed input inventory. The
    // evaluator has already retained all ordered argument effects in context;
    // applying those effects again here would retarget an earlier descriptor.
    let argument = |index: usize| arguments.get(index);
    let substitute = |root, segments: &[PlaceSegment], kind| {
        let index = typed_trees::signature::state_parameter_position(parameters, root)?;
        let captured = argument(index)?;
        destination_sources(
            program,
            machine,
            &parameters[index],
            captured,
            segments,
            kind,
            context,
            symbols,
        )
    };
    let mut output = Vec::new();
    let mut substitutions = Vec::new();
    for effect in relative {
        // An identity row proves preservation; it has no store to instantiate.
        // In particular, a wildcard default is not an executable selector.
        if effect.incoming && effect.sources.as_ref()?.is_empty() {
            continue;
        }
        let Some(destinations) = substitute(effect.root, &effect.segments, effect.kind) else {
            trace_instantiation_refusal(
                machine,
                parameters,
                arguments,
                effect,
                None,
                "destination",
            );
            return None;
        };
        let formal = FrameSourcePlace {
            segments: effect.segments.clone(),
            ..FrameSourcePlace::parameter_root(
                &parameters
                    [typed_trees::signature::state_parameter_position(parameters, effect.root)?],
            )
        };
        for (_, input) in &destinations {
            if !substitutions.contains(&(formal.clone(), input.clone())) {
                substitutions.push((formal.clone(), input.clone()));
            }
        }
        let mut sources = Vec::new();
        for supplied in effect.sources.as_ref()? {
            let Some(observed_sources) = captured_inputs::instantiate_sources(
                program, machine, parameters, arguments, supplied, context, symbols,
            ) else {
                trace_instantiation_refusal(
                    machine,
                    parameters,
                    arguments,
                    effect,
                    Some(supplied),
                    "source",
                );
                return None;
            };
            for observed in observed_sources {
                for substitution in observed.substitutions {
                    if !substitutions.contains(&substitution) {
                        substitutions.push(substitution);
                    }
                }
                if !sources.contains(&observed.source) {
                    sources.push(observed.source);
                }
            }
        }
        let conditional = destinations.len() > 1;
        for (destination, _) in destinations {
            output.push(SlotEffect {
                root: destination.root,
                segments: destination.segments,
                sources: Some(sources.clone()),
                // One exact alternative is chosen at runtime. Preserve the
                // incoming contents of every other enumerated destination.
                incoming: effect.incoming || conditional,
                may_select: effect.may_select,
                kind: effect.kind,
            });
        }
    }
    Some(InstantiatedSlots {
        effects: output,
        substitutions,
    })
}

/// A refused summary remains refused. Keep diagnostics at the substitution
/// owner so a missing destination cannot be mistaken for a source association
/// failure, without exporting complete body inventories or unbounded paths.
fn trace_instantiation_refusal(
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    effect: &SlotEffect,
    source: Option<&SlotSource>,
    reason: &str,
) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
        return;
    }
    eprintln!(
        "CHECKSITE stored-reference-instantiation-refusal machine={:?} reason={} destination={:?}:path_count={}:prefix={:?} incoming={} may_select={} source_count={:?} input_count={} inputs_prefix={:?}",
        machine.symbol,
        reason,
        effect.root,
        effect.segments.len(),
        &effect.segments[..effect.segments.len().min(32)],
        effect.incoming,
        effect.may_select,
        effect.sources.as_ref().map(Vec::len),
        arguments.len(),
        parameters
            .iter()
            .zip(arguments)
            .take(8)
            .map(|(parameter, argument)| (parameter.symbol, argument.expression))
            .collect::<Vec<_>>(),
    );
    if let Some(source) = source {
        eprintln!(
            "CHECKSITE stored-reference-instantiation-source carrier={:?}:path_count={}:prefix={:?} backing={:?}:path_count={}:prefix={:?} precision={:?} lifetime_only={:?} recast_count={} representation={}",
            source.carrier.root,
            source.carrier.segments.len(),
            &source.carrier.segments[..source.carrier.segments.len().min(32)],
            source.origin.source.root,
            source.origin.source.segments.len(),
            &source.origin.source.segments[..source.origin.source.segments.len().min(32)],
            source.origin.precision,
            source.lifetime_access,
            source.origin.source.recasts.len(),
            source.origin.source.representation.is_some(),
        );
    }
}

/// Resolve each pointer boundary before appending the destination slot.
/// Only the selected descriptor is interpreted; the final slot's old contents
/// never become its own storage address.
#[allow(clippy::too_many_arguments)]
pub(super) fn destination_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    segments: &[PlaceSegment],
    kind: stored_origins::StoredOriginKind,
    entry: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<(FrameSourcePlace, FrameSourcePlace)> {
    let mut destinations = destination_sources(
        program, machine, parameter, captured, segments, kind, entry, symbols,
    )?;
    (destinations.len() == 1).then(|| destinations.remove(0))
}

/// Enumerate a finite set of exact addresses. Each crossed descriptor must
/// have a complete captured backing relation; alternatives never authorize a
/// wildcard selector or pair independent carrier/backing observations.
#[allow(clippy::too_many_arguments)]
fn destination_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    segments: &[PlaceSegment],
    kind: stored_origins::StoredOriginKind,
    entry: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<(FrameSourcePlace, FrameSourcePlace)>> {
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
    let mut pending = Vec::new();
    let reference = parameter.type_reference;
    if let Some(shell) = crate::reference_shell(program, reference) {
        if !shell.access.is_exclusive() {
            return None;
        }
        let relative = FrameSourcePlace::parameter_root(parameter);
        let sources = aggregate_argument_sources(
            program,
            machine,
            parameter,
            captured,
            &SlotSource::from_place(program, relative, FramePathPrecision::Exact, None),
            entry,
            symbols,
        )?;
        if sources.is_empty() {
            return None;
        }
        let finite_alternatives = sources.len() > 1;
        for source in sources {
            if source.lifetime_access.is_some()
                || (finite_alternatives && source.origin.precision != FramePathPrecision::Exact)
            {
                return None;
            }
            let input = source.origin.source.clone();
            pending.push((
                shell.referee,
                segments,
                Some(source.origin),
                Some(input),
                Vec::new(),
            ));
        }
    } else {
        pending.push((reference, segments, None, None, Vec::new()));
    }
    let mut completed = Vec::new();
    while let Some((reference, segments, base, input, mut ancestors)) = pending.pop() {
        let identity = (reference, segments.to_vec(), base.clone());
        if ancestors.contains(&identity) {
            return None;
        }
        ancestors.push(identity);
        let inventory = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            reference,
            true,
        )?;
        let mut boundaries = inventory.references.iter().filter(|leaf| {
            leaf.kind.may_indirect_storage()
                && leaf.local_segments.len() < segments.len()
                && segments.starts_with(&leaf.local_segments)
        });
        let Some(boundary) = boundaries.next() else {
            let leaf = stored_origins::projected_storage_type(program, reference, segments)?;
            match kind {
                stored_origins::StoredOriginKind::Reference(access) => {
                    let shell = crate::reference_shell(program, leaf)?;
                    (shell.access == access).then_some(())?;
                }
                stored_origins::StoredOriginKind::LifetimeDependency => {
                    // A token's slot is ordinary owned storage, not a pointer
                    // value. Its complete canonical frontier must identify a
                    // logical obligation at this exact projected carrier.
                    crate::borrow_contracts::retained_lifetime_owner_paths(program, leaf)?
                        .iter()
                        .any(Vec::is_empty)
                        .then_some(())?;
                }
                stored_origins::StoredOriginKind::OpaqueRegion => return None,
            }
            let base = base?;
            if !base.source.root.is_valid()
                || base.source.is_static()
                || !base.source.builtin_coordinates
            {
                return None;
            }
            let row = (base.source.append_segments(segments), input?);
            if !completed.contains(&row) {
                completed.push(row);
            }
            continue;
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
        let candidates = if let Some(base) = &base {
            let mut projected = base.clone();
            projected.source = base.source.append_segments(&boundary.local_segments);
            projected.path = path(program, &projected.source);
            stored_origins::captured_reference_projection_origins(
                program,
                base,
                &projected,
                &entry.stored,
            )
            .into_iter()
            .collect::<Option<Vec<_>>>()?
        } else {
            let relative = FrameSourcePlace {
                segments: boundary.local_segments.clone(),
                ..FrameSourcePlace::parameter_root(parameter)
            };
            let sources = aggregate_argument_sources(
                program,
                machine,
                parameter,
                captured,
                &SlotSource::from_place(program, relative, FramePathPrecision::Exact, None),
                entry,
                symbols,
            )?;
            let mut candidates = Vec::new();
            for source in sources {
                if source.lifetime_access.is_some() {
                    return None;
                }
                candidates.push(source.origin);
            }
            candidates
        };
        if candidates.is_empty() {
            return None;
        }
        let finite_alternatives = candidates.len() > 1;
        for selected in candidates {
            if finite_alternatives && selected.precision != FramePathPrecision::Exact {
                return None;
            }
            let input = input.clone().unwrap_or_else(|| selected.source.clone());
            pending.push((
                shell.referee,
                &segments[boundary.local_segments.len()..],
                Some(selected),
                Some(input),
                ancestors.clone(),
            ));
        }
    }
    (!completed.is_empty()).then_some(completed)
}

/// Returned source substitution uses each operand's completed observation.
/// Spatial backing and descriptor carrier stay associated through computed
/// results; no later ambient inventory can reinterpret a prior descriptor.
pub(super) fn aggregate_argument_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    supplied: &SlotSource,
    entry: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<SlotSource>> {
    captured_inputs::sources(
        program, machine, parameter, captured, supplied, entry, symbols,
    )
}
