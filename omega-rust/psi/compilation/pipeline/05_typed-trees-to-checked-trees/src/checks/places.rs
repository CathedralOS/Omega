//! The checked place a recorded root and segment span name: flow keeps segment
//! payloads in shared arenas and records a root plus a span into them.

use arena::{HandleSpan, PagedArenaSet};
use facts::{PlaceRoot, PlaceSegment};
use validation::CanonicalPlace;

/// The exact place the span names. `None` when the span is incomplete: a
/// partial path names no storage.
pub(super) fn place_in(
    segments: &PagedArenaSet<PlaceSegment>,
    root: PlaceRoot,
    span: HandleSpan<PlaceSegment>,
) -> Option<CanonicalPlace> {
    Some(CanonicalPlace {
        root,
        segments: segments.span(span)?.to_vec(),
    })
}

/// The place the span names, reading an incomplete span as the bare root: a
/// write footprint or captured origin admits that place.
pub(super) fn place_or_root_in(
    segments: &PagedArenaSet<PlaceSegment>,
    root: PlaceRoot,
    span: HandleSpan<PlaceSegment>,
) -> CanonicalPlace {
    CanonicalPlace {
        root,
        segments: segments.span_or_empty(span).to_vec(),
    }
}
