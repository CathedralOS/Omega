//! Reference values follow a source leaf, not a may-write overlap closure.
//! An owned field beside a reference cannot acquire that reference's origin
//! merely because indexing made their write footprints identical.

use super::{FramePlaceOrigin, StoredLocalOrigins, compose_origin, place_suffix};
use facts::PlaceSegment;
use typed_trees::TypedTrees;

pub(in crate::calls::write_frames) fn canonical_reference_origins(
    program: &TypedTrees,
    origin: &FramePlaceOrigin,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<FramePlaceOrigin> {
    if !origin.source.root.is_valid() {
        // Existing path-only evidence can still conservatively invalidate
        // storage. It cannot later export an owned-formal reference boundary.
        return super::canonical_origins(origin, aliases, stored);
    }
    let mut origin = origin.clone();
    if let Some((_, prior)) = aliases
        .iter()
        .find(|(name, _)| program.symbols.name(origin.source.root) == name)
    {
        let (_, suffix) = super::split_place_root(&origin.path);
        let source = prior.source.append_segments(&origin.source.segments);
        origin = compose_origin(prior, suffix, origin.precision);
        origin.source = source;
    }
    let Some(local) = stored
        .iter()
        .find(|local| local.local_symbol == origin.source.root)
    else {
        return vec![origin];
    };
    let mut sources = Vec::new();
    for leaf in &local.references {
        if !source_reaches_leaf(&origin.source.segments, &leaf.local_segments) {
            continue;
        }
        let suffix = place_suffix(&leaf.local_path, &origin.path).unwrap_or("");
        let mut source = compose_origin(&leaf.origin, suffix, origin.precision);
        source.source = leaf
            .origin
            .source
            .append_segments(&origin.source.segments[leaf.local_segments.len()..]);
        if !sources.iter().any(|prior: &FramePlaceOrigin| {
            prior.path == source.path
                && prior.precision == source.precision
                && prior.source == source.source
        }) {
            sources.push(source);
        }
    }
    // No reference boundary was crossed. Preserve the private source so
    // exported result analysis rejects it; do not substitute a sibling leaf.
    if sources.is_empty() {
        vec![origin]
    } else {
        sources
    }
}

/// The source must select the leaf or storage beneath it. Ancestor overlap
/// is useful for invalidating facts, never evidence of a reference value.
pub(in crate::calls::write_frames) fn source_reaches_leaf(
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
