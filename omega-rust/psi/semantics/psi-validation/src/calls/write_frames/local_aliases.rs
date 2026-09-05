//! Pure local-alias queries for caller-visible write frames.
//!
//! This leaf rebases relative paths through already-canonical alias origins
//! and detects syntactic mutable reborrows or reference-shaped replacements of
//! stable parameter/local aliases. It also resolves direct place expressions
//! through those established origins. It neither recursively infers origins,
//! mutates alias bindings, nor resolves call frames.

use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path, split_place_root,
};
use super::type_capabilities::type_reference_is_reference;
use crate::arithmetic_domains;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;

pub(super) fn rebase_local_alias_path(
    relative: &str,
    aliases: &[(String, FramePlaceOrigin)],
) -> String {
    let (root, suffix) = split_place_root(relative);
    aliases
        .iter()
        .find_map(|(alias, origin)| {
            (alias == root).then(|| match origin.precision {
                FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
                FramePathPrecision::CollectionCoarse => origin.path.clone(),
            })
        })
        .unwrap_or_else(|| relative.to_owned())
}

/// Resolve one direct typed place through already-established parameter,
/// caller-isolated-local, or stable-alias origins. Exact aliases compose the
/// authored suffix; a collection-coarse origin remains coarse and cannot be
/// narrowed by a later member projection.
pub(super) fn stable_alias_place_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    };
    let origin = frame_place_path(program, expression)?;
    let (root, suffix) = split_place_root(&origin.path);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
        || (allow_isolated_local && isolated_local_roots.iter().any(|local| local == root))
    {
        return Some(origin);
    }
    let parent = aliases
        .iter()
        .find_map(|(alias, parent)| (alias == root).then_some(parent))?;
    if !allow_isolated_local
        && isolated_local_roots
            .iter()
            .any(|local| local == split_place_root(&parent.path).0)
    {
        return None;
    }
    Some(match parent.precision {
        FramePathPrecision::Exact => FramePlaceOrigin {
            path: append_place_suffix(&parent.path, suffix),
            precision: origin.precision,
        },
        FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
            path: parent.path.clone(),
            precision: FramePathPrecision::CollectionCoarse,
        },
    })
}

pub(super) fn expression_reborrows_local_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    expression_reborrows_reference_binding(program, expression, &|target| {
        arithmetic_domains::place_path(program, target)
            .is_some_and(|path| aliases.iter().any(|(alias, _)| path == *alias))
    })
}

pub(super) fn expression_reborrows_stable_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    expression_reborrows_reference_binding(program, expression, &|target| {
        frame_place_path(program, target).is_some_and(|place| {
            let (root, suffix) = split_place_root(&place.path);
            suffix.is_empty()
                && (parameters.iter().any(|parameter| {
                    super::reference_origins::exclusive_reference_referee(
                        program,
                        parameter.type_reference,
                    )
                    .is_some()
                        && (parameter.is_self && root == "self" || root == parameter.name.as_str())
                }) || aliases.iter().any(|(name, _)| root == name))
        })
    })
}

pub(super) fn expression_reborrows_reference_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    is_reference_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    expression_has_exclusive_borrow(program, expression, &|target| {
        matches!(
            program.expression_table.expression(target),
            ExpressionNode::Name(_)
        ) && is_reference_binding(target)
    })
}

pub(super) fn expression_has_exclusive_borrow(
    program: &TypedTrees,
    expression: ExpressionHandle,
    is_reference_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() {
            continue;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => {
                if inner.access.is_exclusive() && is_reference_binding(inner.target) {
                    return true;
                }
                pending.push(inner.target);
            }
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}

/// A bare write through `alias` (`alias = 1`) targets the borrowed place, but
/// Psi also permits a mutable-reference local declared with plain `let` to be
/// rebound (`alias = &mut other`). Accept an exact origin only while the RHS is
/// proven value-shaped; unknown/reference-shaped replacements fail closed.
pub(super) fn expression_may_rebind_mutable_alias(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(_) => true,
        ExpressionNode::Call(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_) => {
            crate::places::expression_result_is_reference(program, machine, state, expression)
                .unwrap_or(true)
        }
        ExpressionNode::Cast(cast) => type_reference_is_reference(program, cast.target_type),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
