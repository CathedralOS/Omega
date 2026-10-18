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
    if captured.source.root != projected.source.root
        || captured.source.is_static() != projected.source.is_static()
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
            source.and_then(|source| (!source.lifetime_access.is_some()).then_some(source.origin))
        })
        .collect()
}

fn spatial_source(origin: FramePlaceOrigin) -> Option<SlotSource> {
    Some(SlotSource {
        carrier: origin.source.clone(),
        origin,
        lifetime_access: None,
    })
}

fn resolve_sources(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    mut crossed: Option<usize>,
) -> Vec<Option<SlotSource>> {
    let Some(origin) = origin else {
        return vec![None];
    };
    if origin.source.is_static() {
        return vec![spatial_source(origin.clone())];
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
            if !sources.contains(&Some(dependency.clone())) {
                sources.push(Some(dependency.clone()));
            }
        }
        let Some(mut captured) = leaf.origin.clone() else {
            if leaf.lifetime_dependencies.is_empty() && !sources.contains(&None) {
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
        let source = spatial_source(source);
        if !sources.contains(&source) {
            sources.push(source);
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
