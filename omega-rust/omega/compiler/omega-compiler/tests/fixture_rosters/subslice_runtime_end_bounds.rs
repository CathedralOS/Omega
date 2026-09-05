//! Named fixtures executed by the dedicated host-native subslice tests.
//! Inventory membership does not change their exit-status or target contracts.

pub(crate) const DOMAINED_SLICE_LEN_GUARD_EXIT: &str = "slices/domained_slice_len_guard_exit";
pub(crate) const RUNTIME_END_SUBSLICE_MACHINE_FIELD_EXIT: &str =
    "slices/runtime_end_subslice_machine_field_exit";
pub(crate) const DOMAINED_RUNTIME_END_SUBSLICE_EXIT: &str =
    "slices/domained_runtime_end_subslice_exit";

// Consumed by corpus inventory; the dedicated target uses named cases directly.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    DOMAINED_SLICE_LEN_GUARD_EXIT,
    RUNTIME_END_SUBSLICE_MACHINE_FIELD_EXIT,
    DOMAINED_RUNTIME_END_SUBSLICE_EXIT,
];
