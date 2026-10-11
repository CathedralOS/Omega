//! Captured view coordinates share their boundary dimension. These helpers
//! compose established coordinates only; they neither evaluate selectors nor
//! establish access, origin, extent or bounds evidence.

use super::PlaceSegment;

/// Compose adjacent address-window coordinates in one captured address.
/// Pointwise paths through nested values must instead use
/// `project_captured_view_segments`, which joins only the boundary dimension.
pub fn compose_captured_view_segments(segments: &mut Vec<PlaceSegment>) -> Option<()> {
    let mut composed = Vec::with_capacity(segments.len());
    for &segment in segments.iter() {
        if let Some(previous) = composed.last().copied()
            && let Some(combined) = compose_boundary(previous, segment)?
        {
            *composed.last_mut()? = combined;
        } else {
            composed.push(segment);
        }
    }
    *segments = composed;
    Some(())
}

/// Project a value path below a captured address. Only the final captured
/// coordinate and the first projected coordinate can share a collection
/// dimension; later arrays remain distinct even when their ranges are adjacent.
pub fn project_captured_view_segments(
    captured: &[PlaceSegment],
    projection: &[PlaceSegment],
) -> Option<Vec<PlaceSegment>> {
    let mut segments = captured.to_vec();
    let Some((&first, remaining)) = projection.split_first() else {
        return Some(segments);
    };
    // A declaration's whole-slice range denotes the captured extent, rather
    // than an address interval of usize::MAX elements.
    let first = match (segments.last(), first) {
        (
            Some(PlaceSegment::FixedRange { start, end }),
            PlaceSegment::FixedRange {
                start: 0,
                end: usize::MAX,
            },
        ) => PlaceSegment::FixedRange {
            start: 0,
            end: end.checked_sub(*start)?,
        },
        _ => first,
    };
    if let Some(previous) = segments.last().copied()
        && let Some(combined) = compose_boundary(previous, first)?
    {
        *segments.last_mut()? = combined;
    } else {
        segments.push(first);
    }
    segments.extend_from_slice(remaining);
    Some(segments)
}

fn compose_boundary(previous: PlaceSegment, next: PlaceSegment) -> Option<Option<PlaceSegment>> {
    Some(match (previous, next) {
        (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index }) => {
            if index >= end.checked_sub(start)? {
                return None;
            }
            Some(PlaceSegment::FixedIndex {
                index: start.checked_add(index)?,
            })
        }
        (
            PlaceSegment::FixedRange { start, end },
            PlaceSegment::FixedRange {
                start: next_start,
                end: next_end,
            },
        ) => {
            if next_start > next_end || next_end > end.checked_sub(start)? {
                return None;
            }
            Some(PlaceSegment::FixedRange {
                start: start.checked_add(next_start)?,
                end: start.checked_add(next_end)?,
            })
        }
        _ => None,
    })
}
