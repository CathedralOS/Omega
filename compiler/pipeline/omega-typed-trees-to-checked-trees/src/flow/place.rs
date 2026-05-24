mod comparison;
mod construction;
mod resolution;

use super::*;
pub(crate) use comparison::{
    canonical_place_overlaps_joined_segments, canonical_place_overlaps_segments,
    canonical_place_segments_equal,
};
pub(crate) use construction::{
    canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place,
    canonical_place_from_symbol,
};
pub(crate) use resolution::{
    effective_member_symbol, expression_type_symbol, symbol_type_symbol,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalPlace {
    pub(crate) root: omega_facts::PlaceRoot,
    pub(crate) segments: Vec<omega_facts::PlaceSegment>,
}

impl CanonicalPlace {
    pub(crate) fn extend_segments(&mut self, segments: &[omega_facts::PlaceSegment]) {
        self.segments.extend(segments.iter().copied());
    }
}
