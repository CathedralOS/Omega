//! Sources shared by the dedicated layout oracles and corpus inventory.
//! Native targets, interpreter checks, and byte expectations stay with the tests.

pub(crate) const PRIMITIVE_CANARY: &str = "layouts/runtime_plan_laid_tiled_outer_array_view_exit";
pub(crate) const RECORD_CANARY: &str = "layouts/runtime_plan_laid_tiled_record_array_view_exit";
pub(crate) const NESTED_ARRAY_CANARY: &str =
    "layouts/runtime_plan_laid_tiled_nested_array_view_exit";
pub(crate) const RECORD_NESTED_ARRAY_CANARY: &str =
    "layouts/runtime_plan_laid_tiled_record_nested_array_view_exit";
pub(crate) const MULTIPLE_AGGREGATE_FIELDS_CANARY: &str =
    "layouts/runtime_plan_laid_multiple_aggregate_fields_view_exit";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    PRIMITIVE_CANARY,
    RECORD_CANARY,
    NESTED_ARRAY_CANARY,
    RECORD_NESTED_ARRAY_CANARY,
    MULTIPLE_AGGREGATE_FIELDS_CANARY,
];
