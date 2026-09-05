//! Exact corpus inputs consumed by the recast views test owner.
//! Compilation phases, targets, and differential assertions remain in the tests.

pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_domain_recast_exit";
pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_range_recast_exit";
pub(crate) const RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_bool_representation_recast_exit";
pub(crate) const RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT: &str =
    "recast/runtime_shared_domain_weakening_recast_exit";
pub(crate) const RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_float_range_representation_recast_exit";
pub(crate) const RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT: &str =
    "recast/runtime_shared_record_float_range_weakening_exit";
pub(crate) const RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT: &str =
    "recast/runtime_mutable_equivalent_record_recast_exit";
pub(crate) const RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT: &str =
    "recast/runtime_aggregate_slice_representation_recast_exit";
pub(crate) const RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT: &str =
    "recast/runtime_interior_slice_view_mutable_write_exit";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT,
    RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT,
    RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT,
    RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT,
    RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT,
    RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT,
    RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT,
    RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT,
    RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT,
];
