//! Reference values follow a source leaf, not a may-write overlap closure.
//! An owned field beside a reference cannot acquire that reference's origin
//! merely because indexing made their write footprints identical.

use super::super::stored_calls::SlotSource;
use super::{FramePlaceOrigin, StoredLocalOrigins, compose_origin, place_suffix};
use facts::PlaceSegment;

pub(in crate::machine_calls::calls::write_frames) fn canonical_reference_origins(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    spatial_origins(resolve_sources(program, origin, aliases, stored, None))
}

pub(in crate::machine_calls::calls::write_frames) fn canonical_reference_sources(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<SlotSource>> {
    resolve_sources(program, origin, aliases, stored, None)
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_projection_origins(
    program: &dyn crate::CheckingInput,
    captured: &FramePlaceOrigin,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    spatial_origins(captured_reference_projection_sources(
        program, captured, projected, stored,
    ))
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_projection_sources(
    program: &dyn crate::CheckingInput,
    captured: &FramePlaceOrigin,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Vec<Option<SlotSource>> {
    if !captured.source.same_namespace(&projected.source)
        || !projected
            .source
            .segments
            .starts_with(&captured.source.segments)
    {
        return vec![None];
    }
    resolve_sources(
        program,
        &Some(projected.clone()),
        &[],
        stored,
        Some(captured.source.segments.len()),
    )
}

fn spatial_origins(sources: Vec<Option<SlotSource>>) -> Vec<Option<FramePlaceOrigin>> {
    sources
        .into_iter()
        .map(|source| {
            source.and_then(|source| {
                (source.lifetime_access.is_none()
                    && source.origin.source.output_identity().is_none())
                .then_some(source.origin)
            })
        })
        .collect()
}

/// A loaded slot records the captured backing-prefix depth independently of
/// the final projection. Following another pointer must skip that already
/// observed descriptor and retain the actual slot which supplied the new one.
struct ReferenceStep {
    source: SlotSource,
    backing_depth: Option<usize>,
}

fn spatial_source(origin: FramePlaceOrigin) -> Option<ReferenceStep> {
    Some(ReferenceStep {
        source: SlotSource {
            carrier_backing: None,
            carrier: origin.source.clone(),
            origin,
            lifetime_access: None,
        },
        backing_depth: None,
    })
}

fn resolve_sources(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    crossed: Option<usize>,
) -> Vec<Option<SlotSource>> {
    resolve_steps(program, origin, aliases, stored, crossed)
        .into_iter()
        .map(|step| {
            step.map(|mut step| {
                // Spatial callers historically retain only the resulting backing.
                // The completed-value query below consumes the full step instead,
                // including the descriptor row which authenticated a pointer load.
                if step.backing_depth.is_some() && step.source.lifetime_access.is_none() {
                    step.source.carrier = step.source.origin.source.clone();
                    step.source.carrier_backing = None;
                }
                step.source
            })
        })
        .collect()
}

/// Complete a callee-relative projection of an already observed descriptor.
/// The descriptor value is fixed; each further slot load uses this invocation's
/// entry inventory. Returned carrier coordinates come from the selected stored
/// row, never by appending a suffix to an earlier descriptor's carrier.
pub(in crate::machine_calls::calls::write_frames) fn completed_reference_projection_sources(
    program: &dyn crate::CheckingInput,
    captured: &SlotSource,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<SlotSource>> {
    if !captured.origin.source.same_namespace(&projected.source)
        || !projected
            .source
            .segments
            .starts_with(&captured.origin.source.segments)
    {
        return None;
    }
    let mut pending = vec![(
        SlotSource {
            origin: projected.clone(),
            carrier_backing: captured.carrier_backing.clone().or_else(|| {
                captured
                    .lifetime_access
                    .is_none()
                    .then(|| captured.origin.clone())
            }),
            ..captured.clone()
        },
        captured.origin.source.segments.len(),
        Vec::new(),
    )];
    let mut completed = Vec::new();
    while let Some((source, crossed, mut ancestors)) = pending.pop() {
        if source.origin.source.segments.len() == crossed || source.lifetime_access.is_some() {
            if !completed.contains(&source) {
                completed.push(source);
            }
            continue;
        }
        let identity = (source.origin.clone(), crossed);
        if ancestors.contains(&identity) {
            return None;
        }
        ancestors.push(identity);
        for step in resolve_steps(
            program,
            &Some(source.origin.clone()),
            &[],
            stored,
            Some(crossed),
        ) {
            let mut step = step?;
            if let Some(depth) = step.backing_depth {
                step.source.lifetime_access =
                    step.source.lifetime_access.or(source.lifetime_access);
                pending.push((step.source, depth, ancestors.clone()));
            } else {
                if step.source.lifetime_access.is_none() {
                    step.source.carrier = source.carrier.clone();
                    step.source.carrier_backing = source.carrier_backing.clone();
                }
                if !completed.contains(&step.source) {
                    completed.push(step.source);
                }
            }
        }
    }
    (!completed.is_empty()).then_some(completed)
}

/// The supplied leaf fixes one opaque pointer. Owned projections beneath
/// that pointer keep the same namespace; another descriptor load needs its
/// independently captured contents and cannot be inferred from a suffix.
fn opaque_output_projection_known(
    program: &dyn crate::CheckingInput,
    source: &super::super::place_paths::FrameSourcePlace,
) -> bool {
    let Some(identity) = source.output_identity() else {
        return false;
    };
    if !source.recasts.is_empty() || source.representation.is_some() || !source.builtin_coordinates
    {
        return false;
    }
    let Some(parameter) = crate::invocation_output_parameter(program, identity) else {
        return false;
    };
    let Some(outer) = crate::reference_shell(program, parameter.type_reference) else {
        return false;
    };
    for depth in 0..=source.segments.len() {
        let Some(reference) = crate::owned_projection_type_reference(
            program,
            outer.referee,
            &source.segments[..depth],
        ) else {
            return false;
        };
        if let Some(shell) = crate::reference_shell(program, reference) {
            if depth == source.segments.len() {
                return true;
            }
            return crate::owned_projection_type_reference(
                program,
                shell.referee,
                &source.segments[depth..],
            )
            .is_some_and(|reference| crate::reference_shell(program, reference).is_none());
        }
    }
    false
}

fn resolve_steps(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    mut crossed: Option<usize>,
) -> Vec<Option<ReferenceStep>> {
    let Some(origin) = origin else {
        return vec![None];
    };
    if origin.source.is_static() {
        return vec![spatial_source(origin.clone())];
    }
    if origin.source.output_identity().is_some() {
        return vec![
            opaque_output_projection_known(program, &origin.source)
                .then(|| spatial_source(origin.clone()))
                .flatten(),
        ];
    }
    if !origin.source.root.is_valid() {
        // Existing path-only evidence can still conservatively invalidate
        // storage. It cannot later export an owned-formal reference boundary.
        return super::canonical_origins(program, origin, aliases, stored)
            .map(|origins| origins.into_iter().map(spatial_source).collect())
            .unwrap_or_else(|| vec![None]);
    }
    let mut origin = origin.clone();
    if let Some((_, prior)) = aliases
        .iter()
        .find(|(name, _)| program.symbols.name(origin.source.root) == name)
    {
        crossed = Some(prior.source.segments.len());
        let (_, suffix) = super::split_place_root(&origin.path);
        let source = prior.source.append_source(&origin.source);
        origin = compose_origin(program, prior, suffix, origin.precision);
        origin.source = source;
    }
    let Some(local) = stored
        .iter()
        .find(|local| local.local_symbol == origin.source.root)
    else {
        return vec![spatial_source(origin)];
    };
    // A retained nested slot records its current contents. Its incoming
    // ancestor descriptor only identifies storage and must not reintroduce
    // the pre-store value beside that current slot (including unknown rows).
    let selected_depth = local
        .references
        .iter()
        .filter(|leaf| {
            matches!(leaf.kind, super::StoredOriginKind::Reference(_))
                && !crossed.is_some_and(|depth| leaf.local_segments.len() <= depth)
                && source_reaches_leaf(&origin.source.segments, &leaf.local_segments)
                && !super::projections::reference_is_shadowed(local, leaf, &origin.source.segments)
        })
        .map(|leaf| leaf.local_segments.len())
        .max();
    let mut sources = Vec::new();
    for leaf in &local.references {
        if !matches!(leaf.kind, super::StoredOriginKind::Reference(_))
            || Some(leaf.local_segments.len()) != selected_depth
            || !source_reaches_leaf(&origin.source.segments, &leaf.local_segments)
            || super::projections::reference_is_shadowed(local, leaf, &origin.source.segments)
        {
            continue;
        }
        let suffix = place_suffix(&leaf.local_path, &origin.path).unwrap_or("");
        // Captured lifetime debts survive projection as a whole. Appending
        // output selectors would invent geometry inside an unrelated input.
        for dependency in &leaf.lifetime_dependencies {
            if !sources.iter().any(|step: &Option<ReferenceStep>| {
                step.as_ref().is_some_and(|step| step.source == *dependency)
            }) {
                sources.push(Some(ReferenceStep {
                    source: dependency.clone(),
                    backing_depth: None,
                }));
            }
        }
        let Some(mut captured) = leaf.origin.clone() else {
            if leaf.lifetime_dependencies.is_empty() && !sources.iter().any(Option::is_none) {
                sources.push(None);
            }
            continue;
        };
        if captured.source.root == local.local_symbol
            && captured.source.segments == leaf.local_segments
        {
            for (segment, selected) in captured
                .source
                .segments
                .iter_mut()
                .zip(&origin.source.segments)
            {
                if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid()) {
                    *segment = *selected;
                }
            }
        }
        let mut source = compose_origin(program, &captured, suffix, origin.precision);
        // Loading a stored reference substitutes its captured referent before
        // any subsequent view. Trim storage-slot coordinates, never the logical
        // selectors inside a presentation, and retain every formation event.
        let mut relative = origin.source.clone();
        relative.segments.drain(..leaf.local_segments.len());
        for recast in &mut relative.recasts {
            if recast.root != origin.source.root
                || !source_reaches_leaf(&recast.segments, &leaf.local_segments)
            {
                return vec![None];
            }
            recast.segments.drain(..leaf.local_segments.len());
        }
        source.source = captured.source.append_source(&relative);
        let removed = origin.source.segments.len() - leaf.local_segments.len();
        let mut carrier = origin.source.clone();
        carrier.segments.truncate(leaf.local_segments.len());
        if let Some((_, selectors)) = &mut carrier.representation {
            let Some(length) = selectors.len().checked_sub(removed) else {
                return vec![None];
            };
            selectors.truncate(length);
        }
        carrier.recasts.retain(|recast| {
            recast.root != carrier.root || recast.segments.len() <= carrier.segments.len()
        });
        let backing_depth = captured.source.segments.len();
        let step = ReferenceStep {
            source: SlotSource {
                carrier_backing: Some(captured),
                origin: source,
                carrier,
                lifetime_access: None,
            },
            backing_depth: Some(backing_depth),
        };
        if !sources.iter().any(|prior| {
            prior.as_ref().is_some_and(|prior| {
                prior.source == step.source && prior.backing_depth == step.backing_depth
            })
        }) {
            sources.push(Some(step));
        }
    }
    // No reference boundary was crossed. Preserve the private source so
    // exported result analysis rejects it; do not substitute a sibling leaf.
    if sources.is_empty() {
        vec![spatial_source(origin)]
    } else {
        sources
    }
}

/// The source must select the leaf or storage beneath it. Ancestor overlap
/// is useful for invalidating facts, never evidence of a reference value.
pub(in crate::machine_calls::calls::write_frames) fn source_reaches_leaf(
    source: &[PlaceSegment],
    leaf: &[PlaceSegment],
) -> bool {
    source.len() >= leaf.len()
        && source.iter().zip(leaf).all(|(source, leaf)| {
            source == leaf
                || matches!(
                    (source, leaf),
                    (
                        PlaceSegment::Index { .. },
                        PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
                    ) | (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { .. })
                )
        })
}
