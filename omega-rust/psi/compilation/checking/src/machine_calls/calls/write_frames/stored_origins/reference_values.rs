//! Reference values follow a source leaf, not a may-write overlap closure.
//! An owned field beside a reference cannot acquire that reference's origin
//! merely because indexing made their write footprints identical.

use super::{FramePlaceOrigin, StoredLocalOrigins, compose_origin, place_suffix};
use facts::PlaceSegment;
use typed_trees::TypedTrees;

pub(in crate::machine_calls::calls::write_frames) fn canonical_reference_origins(
    program: &TypedTrees,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    resolve_origins(program, origin, aliases, stored, None)
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_projection_origins(
    program: &TypedTrees,
    captured: &FramePlaceOrigin,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    if captured.source.root != projected.source.root
        || captured.source.is_static() != projected.source.is_static()
        || !projected
            .source
            .segments
            .starts_with(&captured.source.segments)
    {
        return vec![None];
    }
    resolve_origins(
        program,
        &Some(projected.clone()),
        &[],
        stored,
        Some(captured.source.segments.len()),
    )
}

fn resolve_origins(
    program: &TypedTrees,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    mut crossed: Option<usize>,
) -> Vec<Option<FramePlaceOrigin>> {
    let Some(origin) = origin else {
        return vec![None];
    };
    if origin.source.is_static() {
        return vec![Some(origin.clone())];
    }
    if !origin.source.root.is_valid() {
        // Existing path-only evidence can still conservatively invalidate
        // storage. It cannot later export an owned-formal reference boundary.
        return super::canonical_origins(origin, aliases, stored)
            .map(|origins| origins.into_iter().map(Some).collect())
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
        origin = compose_origin(prior, suffix, origin.precision);
        origin.source = source;
    }
    let Some(local) = stored
        .iter()
        .find(|local| local.local_symbol == origin.source.root)
    else {
        return vec![Some(origin)];
    };
    let mut sources = Vec::new();
    for leaf in &local.references {
        if crossed.is_some_and(|depth| leaf.local_segments.len() <= depth)
            || !source_reaches_leaf(&origin.source.segments, &leaf.local_segments)
        {
            continue;
        }
        let suffix = place_suffix(&leaf.local_path, &origin.path).unwrap_or("");
        let Some(captured) = &leaf.origin else {
            if !sources.contains(&None) {
                sources.push(None);
            }
            continue;
        };
        let mut source = compose_origin(captured, suffix, origin.precision);
        source.source = captured
            .source
            .append_segments(&origin.source.segments[leaf.local_segments.len()..]);
        source.source.builtin_coordinates &= origin.source.builtin_coordinates;
        if !sources.contains(&Some(source.clone())) {
            sources.push(Some(source));
        }
    }
    // No reference boundary was crossed. Preserve the private source so
    // exported result analysis rejects it; do not substitute a sibling leaf.
    if sources.is_empty() {
        vec![Some(origin)]
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
