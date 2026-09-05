//! Frame-place path algebra for complete-or-opaque write summaries.
//!
//! Indexed write footprints deliberately coarsen to their collection. That
//! loss is absorbing in the string path; structural source selectors remain
//! separate and retain later fields. This leaf owns only path recovery and
//! composition; it performs no call resolution or frame traversal.

use crate::arithmetic_domains;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod source_places;

pub(super) use source_places::FrameSourcePlace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FramePathPrecision {
    Exact,
    CollectionCoarse,
}

#[derive(Debug, Clone)]
pub(super) struct FramePlaceOrigin {
    pub(super) path: String,
    pub(super) precision: FramePathPrecision,
    pub(super) source: FrameSourcePlace,
}

pub(super) fn split_place_root(path: &str) -> (&str, &str) {
    let boundary = path.find(['.', '[']).unwrap_or(path.len());
    path.split_at(boundary)
}

pub(super) fn append_place_suffix(base: &str, suffix: &str) -> String {
    format!("{base}{suffix}")
}

/// Coarsen indexed writes to their collection (`self.cells[i]` writes
/// `self.cells`). The string/path frame algebra deliberately does not retain
/// element identity; structured mutation places own exact fixed-index facts.
pub(super) fn coarse_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    raw_frame_place_path(program, expression).map(|(path, _)| path)
}

/// Recover a frame path together with whether indexing discarded element
/// identity. Collection-coarse paths are absorbing: callers must not append a
/// callee/member suffix and accidentally manufacture `self.cells.value` from
/// a write through `self.cells[i].value`.
pub(super) fn frame_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    let (path, precision) = raw_frame_place_path(program, expression)?;
    Some(FramePlaceOrigin {
        path,
        precision,
        source: FrameSourcePlace::from_expression(program, expression),
    })
}

/// String-only callers do not need a structural plan. Recover the coarse
/// footprint first and normalize source selectors once, only when requested.
fn raw_frame_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, FramePathPrecision)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => raw_frame_place_path(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            let (collection, _) = raw_frame_place_path(program, indexed.collection)?;
            Some((collection, FramePathPrecision::CollectionCoarse))
        }
        ExpressionNode::Member(member) => {
            let (mut receiver, precision) = raw_frame_place_path(program, member.receiver)?;
            if precision == FramePathPrecision::Exact {
                receiver.push('.');
                receiver.push_str(member.member.as_str());
            }
            Some((receiver, precision))
        }
        _ => Some((
            arithmetic_domains::place_path(program, expression)?,
            FramePathPrecision::Exact,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionNode, TableIndexedExpression, TableNamePath};
    use typed_trees::name::Identifier;

    fn name(program: &mut TypedTrees, spelling: &str) -> ExpressionHandle {
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated(spelling));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, SymbolHandle::invalid());
        program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            }))
    }

    #[test]
    fn literal_index_is_collection_coarse_in_string_frame_path() {
        let mut program = TypedTrees::default();
        let collection = name(&mut program, "bytes");
        let index = program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(2),
        ));
        let indexed =
            program
                .expression_table
                .insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }));

        let path = frame_place_path(&program, indexed).expect("literal indexed place");
        assert_eq!(path.path, "bytes");
        assert_eq!(path.precision, FramePathPrecision::CollectionCoarse);
    }

    #[test]
    fn dynamic_index_remains_collection_coarse() {
        let mut program = TypedTrees::default();
        let collection = name(&mut program, "bytes");
        let index = name(&mut program, "index");
        let indexed =
            program
                .expression_table
                .insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }));

        let path = frame_place_path(&program, indexed).expect("dynamic indexed place");
        assert_eq!(path.path, "bytes");
        assert_eq!(path.precision, FramePathPrecision::CollectionCoarse);
    }
}
