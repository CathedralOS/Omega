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
use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::transparent_results::transparent_place_expression_origins;
use language_core::is_self_receiver;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;

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
    inference: &mut FrameInference,
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
        inference,
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
    inference: &mut FrameInference,
    argument_origins: Option<&[Option<Vec<FramePlaceOrigin>>]>,
) -> Option<Vec<String>> {
    let (root, suffix) = split_place_root(relative);
    if is_self_receiver(root) {
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
                inference,
            );
        }
        // A proven argument contributes its finite candidate set: a divergent
        // helper result or conditional actual may route the write to any of
        // its named storage, so each origin instantiates its own caller path.
        let bases = argument_origins
            .and_then(|origins| origins.get(argument_index).cloned())
            .flatten()
            .or_else(|| {
                transparent_place_expression_origins(program, argument, symbols, inference)
            })?;
        let mut paths = Vec::new();
        for base in bases {
            let path = match base.precision {
                FramePathPrecision::Exact => append_place_suffix(&base.path, suffix),
                FramePathPrecision::CollectionCoarse => base.path,
            };
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        return Some(paths);
    }
    if locals.iter().any(|local| local == root) {
        return Some(Vec::new());
    }
    // A write whose root is neither local nor a known parameter is externally
    // visible in a way this rung cannot instantiate safely.
    None
}
