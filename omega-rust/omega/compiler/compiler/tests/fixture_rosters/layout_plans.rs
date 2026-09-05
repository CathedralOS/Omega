//! Named corpus inputs used by the dedicated layout-plan tests.
//! Checked-tree and materialization assertions remain with their test owners.

pub(crate) const RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT: &str =
    "layouts/runtime_plan_laid_value_field_exit";
pub(crate) const PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE: &str =
    "layouts/private_callback_slot_demand_compile";
pub(crate) const PRIVATE_CALLBACK_SLOT_WRONG_LAYOUT: &str =
    "layouts/private_callback_slot_wrong_layout";
pub(crate) const PRIVATE_CALLBACK_SLOT_DUPLICATE: &str = "layouts/private_callback_slot_duplicate";
pub(crate) const PRIVATE_CALLBACK_SLOT_AUTHORED_LOOKALIKE: &str =
    "layouts/private_callback_slot_authored_lookalike";
pub(crate) const PRIVATE_CALLBACK_SLOT_UNTAKEN_COMPILE: &str =
    "layouts/private_callback_slot_untaken_compile";
pub(crate) const RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT: &str =
    "layouts/runtime_plan_laid_compact_bits_exit";
pub(crate) const RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_integer_at_total_write_exit";

// The dedicated target uses named cases; corpus inventory consumes this slice.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT,
    PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE,
    PRIVATE_CALLBACK_SLOT_UNTAKEN_COMPILE,
    RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT,
    RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT,
];

// The dedicated target uses named cases; corpus inventory consumes this slice.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const FAIL_CANARIES: &[&str] = &[
    PRIVATE_CALLBACK_SLOT_WRONG_LAYOUT,
    PRIVATE_CALLBACK_SLOT_DUPLICATE,
    PRIVATE_CALLBACK_SLOT_AUTHORED_LOOKALIKE,
];
