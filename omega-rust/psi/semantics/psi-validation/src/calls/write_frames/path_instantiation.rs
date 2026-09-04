//! Caller-namespace instantiation for relative write-frame paths.
//!
//! This leaf substitutes receiver and ordered parameter roots while preserving
//! exact versus collection-coarse origins. It delegates only actual-argument
//! origin recovery to the parent and performs no frame traversal or diagnostic
//! emission.

use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root,
};
use super::transparent_place_expression_origin;
use crate::symbols::TopLevelSymbols;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::signature::StateParameter;

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate_written_path(
    program: &TypedTrees,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Option<String>> {
    instantiate_written_path_with_origins(
        program,
        relative,
        receiver_base,
        parameters,
        arguments,
        locals,
        symbols,
        active_states,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate_written_path_with_origins(
    program: &TypedTrees,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix(receiver_base?, suffix)));
    }
    if let Some(argument_index) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        let argument = *arguments.get(argument_index)?;
        let base = argument_origins
            .and_then(|origins| origins.get(argument_index))
            .and_then(Clone::clone)
            .or_else(|| {
                transparent_place_expression_origin(program, argument, symbols, active_states)
            })?;
        return Some(Some(match base.precision {
            FramePathPrecision::Exact => append_place_suffix(&base.path, suffix),
            FramePathPrecision::CollectionCoarse => base.path,
        }));
    }
    if locals.iter().any(|local| local == root) {
        return Some(None);
    }
    // A write whose root is neither local nor a known parameter is externally
    // visible in a way this rung cannot instantiate safely.
    None
}
