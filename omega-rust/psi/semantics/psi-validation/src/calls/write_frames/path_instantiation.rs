//! Caller-namespace instantiation for relative write-frame paths.
//!
//! This leaf substitutes receiver and ordered parameter roots while preserving
//! exact versus collection-coarse origins. An aggregate footprint can yield
//! several caller paths; an empty set is private by-value storage, not failure.
//! Reference-leaf origin recovery shares the existing body evidence. There is
//! no second frame traversal or diagnostic policy here.

use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root,
};
use super::transparent_place_expression_origin;
use crate::symbols::TopLevelSymbols;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;

pub(super) mod aggregate_arguments;

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate_written_path(
    program: &TypedTrees,
    caller_machine: &Machine,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let receiver_origin = receiver_base.map(|path| FramePlaceOrigin {
        path: path.to_owned(),
        precision: FramePathPrecision::Exact,
        source: Default::default(),
    });
    instantiate_written_path_with_origins(
        program,
        caller_machine,
        relative,
        receiver_origin.as_ref(),
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
    caller_machine: &Machine,
    relative: &str,
    receiver_base: Option<&FramePlaceOrigin>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
) -> Option<Vec<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        let base = receiver_base?;
        return Some(vec![match base.precision {
            FramePathPrecision::Exact => append_place_suffix(&base.path, suffix),
            FramePathPrecision::CollectionCoarse => base.path.clone(),
        }]);
    }
    if let Some((argument_index, parameter)) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find(|(_, parameter)| parameter.name.as_str() == root)
    {
        let argument = *arguments.get(argument_index)?;
        if matches!(
            program.expression_table.expression(argument),
            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
        ) || (matches!(
            program.expression_table.expression(argument),
            ExpressionNode::Call(_)
        ) && !super::type_reference_is_reference(program, parameter.type_reference)
            && !super::type_is_caller_isolated_local(program, parameter.type_reference))
        {
            return aggregate_arguments::written_paths(
                program,
                caller_machine,
                argument,
                parameter.type_reference,
                suffix,
                symbols,
                active_states,
            );
        }
        let base = argument_origins
            .and_then(|origins| origins.get(argument_index))
            .and_then(Clone::clone)
            .or_else(|| {
                transparent_place_expression_origin(program, argument, symbols, active_states)
            })?;
        return Some(vec![match base.precision {
            FramePathPrecision::Exact => append_place_suffix(&base.path, suffix),
            FramePathPrecision::CollectionCoarse => base.path,
        }]);
    }
    if locals.iter().any(|local| local == root) {
        return Some(Vec::new());
    }
    // A write whose root is neither local nor a known parameter is externally
    // visible in a way this rung cannot instantiate safely.
    None
}
