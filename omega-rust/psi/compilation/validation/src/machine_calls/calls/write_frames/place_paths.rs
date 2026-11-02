//! Frame-place path algebra for complete-or-opaque write summaries.
//!
//! Indexed write footprints deliberately coarsen to their collection. That
//! loss is absorbing in the string path; structural source selectors remain
//! separate and retain later fields. `scalar_coordinates` rejoins those
//! captured selectors through normalized recast geometry. This owner performs
//! no call resolution or frame traversal.

use crate::proof_contracts::arithmetic_domains;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod scalar_coordinates;
mod source_places;
mod write_coordinates;
pub use scalar_coordinates::ScalarStorageCoordinate;
pub(super) use write_coordinates::project_write_path;

pub(super) use source_places::FrameSourcePlace;
pub use source_places::{RecastSource, ReferenceViewFormation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FramePathPrecision {
    Exact,
    CollectionCoarse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FramePlaceOrigin {
    pub(super) path: String,
    pub(super) precision: FramePathPrecision,
    pub(super) source: FrameSourcePlace,
}

/// Two conditional arms keep a single origin only when they agree on path,
/// precision, and structural source; a divergent arm must fail closed rather
/// than select one side of the case analysis.
pub(super) fn same_place_origin(first: &FramePlaceOrigin, second: &FramePlaceOrigin) -> bool {
    first == second
}

/// A proven candidate set keeps one entry per distinct storage route. Two
/// candidates identical in path, precision, and structural source describe
/// the same route; anything else remains a separate possible referent. The
/// union is exact: consumers may treat the whole set as covering every proven
/// route, and any route that fails to resolve fails the expression instead.
pub(super) fn push_unique_origin(origins: &mut Vec<FramePlaceOrigin>, origin: FramePlaceOrigin) {
    if !origins
        .iter()
        .any(|existing| same_place_origin(existing, &origin))
    {
        origins.push(origin);
    }
}

/// Collapse a proven candidate set to the one storage place every route
/// agrees on. Candidates naming the same place and structural source merge,
/// coarsening when their path precision differs. Candidates whose structural
/// sources differ name different storage even under one coarse collection
/// path (`others[0]` and `others[1]`): keeping either source would re-anchor
/// a write to that one element, so they have no single origin and stay with
/// the set consumers, like candidates naming different storage.
pub(super) fn single_place_origin(candidates: Vec<FramePlaceOrigin>) -> Option<FramePlaceOrigin> {
    let mut merged: Option<FramePlaceOrigin> = None;
    for origin in candidates {
        match &mut merged {
            None => merged = Some(origin),
            Some(existing) if existing.path == origin.path && existing.source == origin.source => {
                if origin.precision == FramePathPrecision::CollectionCoarse {
                    existing.precision = FramePathPrecision::CollectionCoarse;
                }
            }
            Some(_) => return None,
        }
    }
    merged
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    raw_frame_place_path(program, expression).map(|(path, _)| path)
}

/// Recover a frame path together with whether indexing discarded element
/// identity. Collection-coarse paths are absorbing: callers must not append a
/// callee/member suffix and accidentally manufacture `self.cells.value` from
/// a write through `self.cells[i].value`.
pub(super) fn frame_place_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    let source = FrameSourcePlace::from_expression(program, expression);
    let raw_root =
        crate::canonical_place_from_expression(program, expression).map(|place| place.root);
    // Symbolic namespace receivers can have frame paths without executable
    // storage coordinates. Only a recovered physical root needs remapping.
    let (path, precision) = if source.root.is_valid()
        && raw_root.is_some_and(|root| root.as_symbol() != Some(source.root))
    {
        coordinate_storage_path(program, program.symbols.name(source.root), &source.segments)
    } else {
        raw_frame_place_path(program, expression)?
    };
    Some(FramePlaceOrigin {
        path,
        precision,
        source,
    })
}

/// String-only callers do not need a structural plan. Recover the coarse
/// footprint first and normalize source selectors once, only when requested.
fn raw_frame_place_path(
    program: &dyn crate::CheckingInput,
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

/// Adapt an authenticated declaration coordinate to the shared caller frame
/// vocabulary. Stored fields remain precise; collection selectors retain the
/// existing conservative containing-region boundary. This supplies no source
/// identity or read permission beyond the caller's selected coordinate.
pub fn frame_coordinate_path(
    program: &dyn crate::CheckingInput,
    root: symbols::SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<String> {
    root.is_valid().then_some(())?;
    Some(coordinate_storage_path(program, program.symbols.name(root), segments).0)
}

/// The frame path of a collection's extent as `.len` observes it, spelled the
/// way an authored `.len` member reaches the frame. A collection-coarse
/// coordinate is absorbing and already covers every element and extent
/// beneath it, so it keeps its coarse spelling.
pub fn frame_length_coordinate_path(
    program: &dyn crate::CheckingInput,
    root: symbols::SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<String> {
    root.is_valid().then_some(())?;
    let (mut path, precision) =
        coordinate_storage_path(program, program.symbols.name(root), segments);
    if precision == FramePathPrecision::Exact {
        path.push_str(".len");
    }
    Some(path)
}

pub(super) fn coordinate_storage_path(
    program: &dyn crate::CheckingInput,
    name: &str,
    segments: &[facts::PlaceSegment],
) -> (String, FramePathPrecision) {
    let mut path = name.to_owned();
    for segment in segments {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                path.push('.');
                path.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::Case { .. } => {}
            _ => return (path, FramePathPrecision::CollectionCoarse),
        }
    }
    (path, FramePathPrecision::Exact)
}
