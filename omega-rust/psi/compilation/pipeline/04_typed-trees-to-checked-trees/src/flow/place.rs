//! Flow-dependent place judgments and semantic-fact translation.
//! Exact typed-program projection lives in validation's value-custody owner;
//! these children narrow live selectors, compare paths and check case access.
mod canonicalization;
mod case_access;
mod comparison;
mod literal_projection;

pub(crate) use facts::compose_captured_view_segments;

pub(crate) use canonicalization::{
    canonical_place_from_semantic_place, canonical_place_from_symbol, narrow_pinned_index_segments,
    projected_leaf_place, reference_source_place, source_independent_place,
};
pub(crate) use case_access::{
    place_case_has_value, place_case_path_is_inactive, place_cases_are_selected,
};
pub(crate) use comparison::{
    canonical_place_joined_segments_may_overlap, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, canonical_places_agree,
    place_segment_has_static_identity, place_segment_has_unresolved_identity,
    static_place_segment_pair_equal,
};
pub(crate) use literal_projection::{
    LiteralExpressionProjection, LiteralValueSource, literal_argument_access_places,
    literal_expression_projections, literal_value_path_is_inactive, literal_value_projections,
};
