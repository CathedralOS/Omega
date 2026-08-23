//! Frame-place path algebra for complete-or-opaque write summaries.
//!
//! Indexed projections deliberately coarsen to their collection. That loss of
//! element identity is absorbing, so later member composition cannot invent a
//! narrower caller-visible path. This leaf owns only path recovery and
//! composition; it performs no call resolution or frame traversal.

use crate::arithmetic_domains;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FramePathPrecision {
    Exact,
    CollectionCoarse,
}

#[derive(Debug, Clone)]
pub(super) struct FramePlaceOrigin {
    pub(super) path: String,
    pub(super) precision: FramePathPrecision,
}

pub(super) fn split_place_root(path: &str) -> (&str, &str) {
    let boundary = path.find(['.', '[']).unwrap_or(path.len());
    path.split_at(boundary)
}

pub(super) fn append_place_suffix(base: &str, suffix: &str) -> String {
    format!("{base}{suffix}")
}

/// Coarsen indexed writes to their collection (`self.cells[i]` writes
/// `self.cells`). The value environment does not track index-sensitive facts.
pub(super) fn coarse_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    Some(frame_place_path(program, expression)?.path)
}

/// Recover a frame path together with whether indexing discarded element
/// identity. Collection-coarse paths are absorbing: callers must not append a
/// callee/member suffix and accidentally manufacture `self.cells.value` from
/// a write through `self.cells[i].value`.
pub(super) fn frame_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => frame_place_path(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            let mut collection = frame_place_path(program, indexed.collection)?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = frame_place_path(program, member.receiver)?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => Some(FramePlaceOrigin {
            path: arithmetic_domains::place_path(program, expression)?,
            precision: FramePathPrecision::Exact,
        }),
    }
}
