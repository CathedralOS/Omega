//! Named source fixtures used by the dedicated recast tests and corpus inventory.
//! Execution stages, target choices, and diagnostic assertions stay in recast_views.rs.

pub(crate) const RUNTIME_SCALAR_PUN_SHARED_LET_EXIT: &str =
    "recast/runtime_scalar_pun_shared_let_exit";
pub(crate) const RUNTIME_INTERIOR_BYTE_RECAST_EXIT: &str =
    "recast/runtime_interior_byte_recast_exit";
pub(crate) const RUNTIME_OFFSET_BYTE_RECAST_EXIT: &str = "recast/runtime_offset_byte_recast_exit";
pub(crate) const RUNTIME_SCALAR_PUN_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_scalar_pun_mutable_write_exit";
pub(crate) const RUNTIME_OFFSET_BYTE_RECAST_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_offset_byte_recast_mutable_write_exit";
pub(crate) const RUNTIME_MULTI_EDGE_OFFSET_MEET_EXIT: &str =
    "recast/runtime_multi_edge_offset_meet_exit";
pub(crate) const RUNTIME_GUARDED_OFFSET_RECAST_EXIT: &str =
    "recast/runtime_guarded_offset_recast_exit";
pub(crate) const RUNTIME_SYMBOLIC_STRIDE_FOOTPRINT_EXIT: &str =
    "recast/runtime_symbolic_stride_footprint_exit";
pub(crate) const RUNTIME_RECORD_VIEW_EXIT: &str = "recast/runtime_record_view_exit";
pub(crate) const RUNTIME_RECORD_ARRAY_VIEW_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_record_array_view_mutable_write_exit";
pub(crate) const CONSTANT_OFFSET_RECORD_VIEW_AFTER_WRITE_EXIT: &str =
    "recast/constant_offset_record_view_after_write_exit";
pub(crate) const RUNTIME_FIXED_ARRAY_VIEW_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_fixed_array_view_mutable_write_exit";
pub(crate) const FIXED_ARRAY_VIEW_FACT_FENCED: &str = "recast/fixed_array_view_fact_fenced";
pub(crate) const RUNTIME_SLICE_VIEW_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_slice_view_mutable_write_exit";
pub(crate) const SLICE_VIEW_NON_TILING_REJECTED: &str = "recast/slice_view_non_tiling_rejected";
pub(crate) const SLICE_VIEW_FACT_FENCED: &str = "recast/slice_view_fact_fenced";
pub(crate) const RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_interior_slice_view_mutable_write_exit";
pub(crate) const INTERIOR_SLICE_RUNTIME_OFFSET_NON_TILING: &str =
    "recast/interior_slice_runtime_offset_non_tiling";
pub(crate) const INTERIOR_SLICE_FACT_FENCED: &str = "recast/interior_slice_fact_fenced";
pub(crate) const RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_aggregate_slice_representation_recast_exit";
pub(crate) const AGGREGATE_SLICE_MUT_LEAF_SETS_DIFFER: &str =
    "recast/aggregate_slice_mut_leaf_sets_differ";
pub(crate) const RECAST_MUT_FACT_FENCED: &str = "recast/recast_mut_fact_fenced";
pub(crate) const RECAST_MUT_INTERIOR_FACT_FENCED: &str = "recast/recast_mut_interior_fact_fenced";
pub(crate) const RECAST_MUT_RECORD_FACT_FENCED: &str = "recast/recast_mut_record_fact_fenced";
pub(crate) const RECAST_MUT_RECORD_ARRAY_FACT_FENCED: &str =
    "recast/recast_mut_record_array_fact_fenced";
pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_domain_recast_exit";
pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_range_recast_exit";
pub(crate) const RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_bool_representation_recast_exit";
pub(crate) const RECAST_SHARED_BOOL_FACT_FENCED: &str = "recast/recast_shared_bool_fact_fenced";
pub(crate) const RECAST_SHARED_INTERIOR_FACT_FENCED: &str =
    "recast/recast_shared_interior_fact_fenced";
pub(crate) const RECAST_MUT_BOOL_BIT_SETS_DIFFER: &str = "recast/recast_mut_bool_bit_sets_differ";
pub(crate) const RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT: &str =
    "recast/runtime_shared_domain_weakening_recast_exit";
pub(crate) const RECAST_SHARED_DOMAIN_STRENGTHENING_REJECTED: &str =
    "recast/recast_shared_domain_strengthening_rejected";
pub(crate) const RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_float_range_representation_recast_exit";
pub(crate) const RECAST_SHARED_FLOAT_RANGE_STRENGTHENING_REJECTED: &str =
    "recast/recast_shared_float_range_strengthening_rejected";
pub(crate) const RECAST_MUT_FLOAT_RANGE_FENCED: &str = "recast/recast_mut_float_range_fenced";
pub(crate) const RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT: &str =
    "recast/runtime_shared_record_float_range_weakening_exit";
pub(crate) const RECAST_SHARED_RECORD_FLOAT_LEAF_STRENGTHENING_REJECTED: &str =
    "recast/recast_shared_record_float_leaf_strengthening_rejected";
pub(crate) const RECAST_MUT_RECORD_FLOAT_LEAF_SETS_DIFFER: &str =
    "recast/recast_mut_record_float_leaf_sets_differ";
pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_record_recast_exit";
pub(crate) const RECAST_MUT_CROSS_CARRIER_DOMAIN_NOT_EQUIVALENT: &str =
    "recast/recast_mut_cross_carrier_domain_not_equivalent";
pub(crate) const RECAST_MUT_RANGE_BIT_SETS_DIFFER: &str = "recast/recast_mut_range_bit_sets_differ";
pub(crate) const RECAST_MUT_RECORD_LEAF_SETS_DIFFER: &str =
    "recast/recast_mut_record_leaf_sets_differ";

// Consumed by corpus inventory; the dedicated target uses named cases directly.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_SCALAR_PUN_SHARED_LET_EXIT,
    RUNTIME_INTERIOR_BYTE_RECAST_EXIT,
    RUNTIME_OFFSET_BYTE_RECAST_EXIT,
    RUNTIME_SCALAR_PUN_MUTABLE_WRITE_EXIT,
    RUNTIME_OFFSET_BYTE_RECAST_MUTABLE_WRITE_EXIT,
    RUNTIME_MULTI_EDGE_OFFSET_MEET_EXIT,
    RUNTIME_GUARDED_OFFSET_RECAST_EXIT,
    RUNTIME_SYMBOLIC_STRIDE_FOOTPRINT_EXIT,
    RUNTIME_RECORD_VIEW_EXIT,
    RUNTIME_RECORD_ARRAY_VIEW_MUTABLE_WRITE_EXIT,
    CONSTANT_OFFSET_RECORD_VIEW_AFTER_WRITE_EXIT,
    RUNTIME_FIXED_ARRAY_VIEW_MUTABLE_WRITE_EXIT,
    RUNTIME_SLICE_VIEW_MUTABLE_WRITE_EXIT,
    RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT,
    RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT,
    RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT,
    RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT,
    RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT,
    RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT,
    RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT,
    RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT,
    RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT,
];

// Consumed by corpus inventory; the dedicated target uses named cases directly.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const FAIL_CANARIES: &[&str] = &[
    FIXED_ARRAY_VIEW_FACT_FENCED,
    SLICE_VIEW_NON_TILING_REJECTED,
    SLICE_VIEW_FACT_FENCED,
    INTERIOR_SLICE_RUNTIME_OFFSET_NON_TILING,
    INTERIOR_SLICE_FACT_FENCED,
    AGGREGATE_SLICE_MUT_LEAF_SETS_DIFFER,
    RECAST_MUT_FACT_FENCED,
    RECAST_MUT_INTERIOR_FACT_FENCED,
    RECAST_MUT_RECORD_FACT_FENCED,
    RECAST_MUT_RECORD_ARRAY_FACT_FENCED,
    RECAST_SHARED_BOOL_FACT_FENCED,
    RECAST_SHARED_INTERIOR_FACT_FENCED,
    RECAST_MUT_BOOL_BIT_SETS_DIFFER,
    RECAST_SHARED_DOMAIN_STRENGTHENING_REJECTED,
    RECAST_SHARED_FLOAT_RANGE_STRENGTHENING_REJECTED,
    RECAST_MUT_FLOAT_RANGE_FENCED,
    RECAST_SHARED_RECORD_FLOAT_LEAF_STRENGTHENING_REJECTED,
    RECAST_MUT_RECORD_FLOAT_LEAF_SETS_DIFFER,
    RECAST_MUT_CROSS_CARRIER_DOMAIN_NOT_EQUIVALENT,
    RECAST_MUT_RANGE_BIT_SETS_DIFFER,
    RECAST_MUT_RECORD_LEAF_SETS_DIFFER,
];
