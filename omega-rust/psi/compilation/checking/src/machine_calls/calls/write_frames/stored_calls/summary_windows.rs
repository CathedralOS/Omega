//! A window of a window stays in the same collection dimension. Recursive
//! normal-exit equations retain may-origins, not the number of times a loop
//! narrowed a descriptor. Keeping that history makes their source lattice
//! infinite. Compose adjacent windows at this summary boundary only;
//! preserve every root, field, element, reference crossing and recast issuer.
//! Opaque selectors grant neither exact coordinates nor descriptor equality.

use super::{FramePlaceOrigin, FrameSourcePlace, SlotSource};
use facts::PlaceSegment;

pub(super) fn coarsen(program: &dyn crate::CheckingInput, source: &mut SlotSource) {
    coarsen_origin(program, &mut source.origin);
    coarsen_place(program, &mut source.carrier);
    if let Some(backing) = &mut source.carrier_backing {
        coarsen_origin(program, backing);
    }
}

fn coarsen_origin(program: &dyn crate::CheckingInput, origin: &mut FramePlaceOrigin) {
    if coarsen_place(program, &mut origin.source) {
        origin.precision = super::super::FramePathPrecision::CollectionCoarse;
    }
}

fn coarsen_place(program: &dyn crate::CheckingInput, place: &mut FrameSourcePlace) -> bool {
    // Only authenticated builtin windows have this algebra. Opaque user-defined
    // indexing retains its existing unsupported/unknown answer.
    if !place.builtin_coordinates {
        return false;
    }
    let is_range = |expression| program.expression_table.range_node(expression).is_some();
    let mut changed = coarsen_path(&mut place.segments, is_range);
    if let Some((_, path)) = &mut place.representation {
        changed |= coarsen_path(path, is_range);
    }
    for recast in &mut place.recasts {
        changed |= coarsen_path(&mut recast.segments, is_range);
        if let Some((_, path)) = &mut recast.representation {
            changed |= coarsen_path(path, is_range);
        }
    }
    changed
}

fn coarsen_path(
    path: &mut Vec<PlaceSegment>,
    is_range: impl Fn(typed_trees::expression::ExpressionHandle) -> bool,
) -> bool {
    let is_window = |part| match part {
        PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => true,
        PlaceSegment::Index { expression } => expression.is_valid() && is_range(expression),
        _ => false,
    };
    let mut written = 0;
    for read in 0..path.len() {
        let part = path[read];
        if written > 0 && is_window(path[written - 1]) && is_window(part) {
            // Compose fixed geometry when both windows prove it; otherwise
            // retain unknown bounds in that original dimension. This cannot cross an
            // element or a field containing another reference descriptor.
            path[written - 1] = match (path[written - 1], part) {
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedRange {
                        start: inner_start,
                        end: inner_end,
                    },
                ) if inner_start <= inner_end
                    && end
                        .checked_sub(start)
                        .is_some_and(|length| inner_end <= length) =>
                {
                    // Valid subwindow endpoints fit within the outer end.
                    PlaceSegment::FixedRange {
                        start: start + inner_start,
                        end: start + inner_end,
                    }
                }
                _ => PlaceSegment::OpaqueRange,
            };
        } else {
            path[written] = part;
            written += 1;
        }
    }
    let changed = written != path.len();
    path.truncate(written);
    changed
}

#[cfg(test)]
mod tests;
