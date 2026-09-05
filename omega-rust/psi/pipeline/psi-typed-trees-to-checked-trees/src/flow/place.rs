mod canonicalization;
mod comparison;
mod contextual;
mod literal_projection;
mod resolution;

use super::*;
pub(crate) use canonicalization::{
    canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place, canonical_place_from_symbol, index_place_segment,
    push_field_place_segments,
};
pub(crate) use comparison::{
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    canonical_place_segments_may_overlap,
};
pub(crate) use contextual::contextual_canonical_place_from_expression;
pub(crate) use literal_projection::{literal_argument_access_places, literal_value_projections};
pub(crate) use resolution::{
    effective_member_symbol, expression_type_symbol, resolve_member_symbol_from_type_symbol,
    symbol_type_symbol,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalPlace {
    pub(crate) root: psi_facts::PlaceRoot,
    pub(crate) segments: Vec<psi_facts::PlaceSegment>,
}

impl CanonicalPlace {
    pub(crate) fn extend_segments(&mut self, segments: &[psi_facts::PlaceSegment]) {
        self.segments.extend(segments.iter().copied());
    }
}
