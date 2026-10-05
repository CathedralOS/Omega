//! Instantiate a returned reference across its validated input-reference
//! boundary. Exact source coordinates survive conservative effect footprints.

use super::super::path_instantiation::aggregate_arguments::{
    AggregateResolver, ReferenceResolver, reference_leaves_with_origins,
};
use super::super::reference_origins::{
    exclusive_reference_origins, exclusive_reference_referee, referent_has_only_owned_storage,
};
use super::super::stored_origins::{declared_origins_for_query, place_suffix, source_reaches_leaf};
use super::super::{
    ExpressionHandle, FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, Machine,
    StateParameter, TopLevelSymbols, TypedTrees, append_place_suffix, split_place_root,
};
use crate::machine_calls::calls::write_frames::FrameInference;

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate_source(
    program: &TypedTrees,
    caller_machine: &Machine,
    callee_machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    relative: &FramePlaceOrigin,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    resolve_reference: &ReferenceResolver<'_>,
    resolve_origins: &AggregateResolver<'_>,
) -> Option<Vec<FramePlaceOrigin>> {
    if include_shared
        && super::super::type_reference_is_reference(program, parameter.type_reference)
    {
        // The result query validated this projection against the helper's
        // frozen binding evidence before leaving its namespace. Its typed
        // source retains fixed indexes even when the effect footprint is
        // collection-coarse. Substitute those coordinates, never recover a
        // reference boundary from the coarsened display path. An unknown or
        // non-builtin source cannot supply the validated boundary.
        if !relative.source.root.is_valid()
            || !relative.source.builtin_coordinates
            || !(relative.source.root == parameter.symbol)
        {
            return None;
        }
        let origins = resolve_reference(actual, parameter.type_reference, true, inference)?;
        let (_, suffix) = split_place_root(&relative.path);
        return Some(
            origins
                .into_iter()
                .map(|origin| compose_source(origin, suffix, relative.precision, &relative.source))
                .collect(),
        );
    }
    if let Some(referee) = exclusive_reference_referee(program, parameter.type_reference) {
        let origins = {
            if !referent_has_only_owned_storage(program, referee) {
                return None;
            }
            exclusive_reference_origins(program, caller_machine, actual, symbols, inference)?
        };
        let (_, suffix) = split_place_root(&relative.path);
        return Some(
            origins
                .into_iter()
                .map(|origin| compose_source(origin, suffix, relative.precision, &relative.source))
                .collect(),
        );
    }
    if super::super::type_reference_is_reference(program, parameter.type_reference)
        || relative.source.root != parameter.symbol
    {
        return None;
    }
    let declared = declared_origins_for_query(
        program,
        parameter.symbol,
        parameter.name.as_str(),
        parameter.type_reference,
        include_shared,
    )?;
    // Prove a reference boundary before looking at actual origins. Otherwise
    // a private owned field can select an unrelated referenced sibling whose
    // string path was coarsened to the same collection.
    let boundary = declared
        .references
        .iter()
        .find(|leaf| source_reaches_leaf(&relative.source.segments, &leaf.local_segments))?;
    let actual_leaves = reference_leaves_with_origins(
        program,
        caller_machine,
        actual,
        parameter.type_reference,
        "",
        symbols,
        inference,
        include_shared,
        resolve_reference,
        resolve_origins,
    )?;
    let mut sources = Vec::new();
    for leaf in actual_leaves.references {
        if !source_reaches_leaf(&relative.source.segments, &leaf.local_segments) {
            continue;
        }
        let suffix = place_suffix(&boundary.local_path, &relative.path).unwrap_or("");
        let remaining = FrameSourcePlace {
            recasts: relative.source.recasts.clone(),
            representation: relative.source.representation.clone(),
            kind: Default::default(),
            root: relative.source.root,
            segments: relative.source.segments[leaf.local_segments.len()..].to_vec(),
            builtin_coordinates: relative.source.builtin_coordinates,
        };
        let source = compose_source(leaf.origin?, suffix, relative.precision, &remaining);
        if !sources.iter().any(|prior: &FramePlaceOrigin| {
            prior.path == source.path
                && prior.precision == source.precision
                && prior.source == source.source
        }) {
            sources.push(source);
        }
    }
    // A selected reference cannot become private just because its selected
    // payload or element supplied no corresponding actual reference leaf.
    (!sources.is_empty()).then_some(sources)
}

fn compose_source(
    mut origin: FramePlaceOrigin,
    suffix: &str,
    precision: FramePathPrecision,
    relative: &FrameSourcePlace,
) -> FramePlaceOrigin {
    if origin.precision == FramePathPrecision::Exact {
        origin.path = append_place_suffix(&origin.path, suffix);
        origin.precision = precision;
    }
    origin.source = origin.source.append_relative(relative);
    origin
}
