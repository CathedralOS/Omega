//! Instantiate a returned reference only across an actual input-reference
//! boundary. A coarse collection footprint cannot supply that boundary.

use super::super::path_instantiation::aggregate_arguments::reference_leaves;
use super::super::reference_origins::{
    exclusive_reference_origin, exclusive_reference_referee, owned_receiver_origin,
    referent_has_only_owned_storage,
};
use super::super::stored_origins::{declared_origins, place_suffix, source_reaches_leaf};
use super::super::{
    ExpressionHandle, FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, Machine,
    StateParameter, TopLevelSymbols, TypedTrees, append_place_suffix, split_place_root,
};
use crate::calls::write_frames::FrameInference;

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
) -> Option<Vec<FramePlaceOrigin>> {
    if let Some(referee) = exclusive_reference_referee(program, parameter.type_reference) {
        let origin = if parameter.is_self {
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == callee_machine.attached_data_symbol)?;
            if !super::super::isolation::data_definition_has_only_owned_storage(program, definition)
            {
                return None;
            }
            owned_receiver_origin(program, caller_machine, actual, symbols, inference)?
        } else {
            if !referent_has_only_owned_storage(program, referee) {
                return None;
            }
            exclusive_reference_origin(program, caller_machine, actual, symbols, inference)?
        };
        let (_, suffix) = split_place_root(&relative.path);
        return Some(vec![compose_source(
            origin,
            suffix,
            relative.precision,
            &relative.source,
        )]);
    }
    if parameter.is_self
        || super::super::type_reference_is_reference(program, parameter.type_reference)
        || relative.source.root != parameter.symbol
    {
        return None;
    }
    let declared = declared_origins(
        program,
        parameter.symbol,
        parameter.name.as_str(),
        parameter.type_reference,
    )?;
    // Prove a reference boundary before looking at actual origins. Otherwise
    // a private owned field can select an unrelated referenced sibling whose
    // string path was coarsened to the same collection.
    let boundary = declared
        .references
        .iter()
        .find(|leaf| source_reaches_leaf(&relative.source.segments, &leaf.local_segments))?;
    let actual_leaves = reference_leaves(
        program,
        caller_machine,
        actual,
        parameter.type_reference,
        "",
        symbols,
        inference,
    )?;
    let mut sources = Vec::new();
    for leaf in actual_leaves.references {
        if !source_reaches_leaf(&relative.source.segments, &leaf.local_segments) {
            continue;
        }
        let suffix = place_suffix(&boundary.local_path, &relative.path).unwrap_or("");
        let remaining = FrameSourcePlace {
            root: relative.source.root,
            segments: relative.source.segments[leaf.local_segments.len()..].to_vec(),
        };
        let source = compose_source(leaf.origin, suffix, relative.precision, &remaining);
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
