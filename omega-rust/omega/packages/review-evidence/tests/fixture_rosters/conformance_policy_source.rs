//! Corpus source repackaged for checked conformance-policy projection tests.

pub(crate) const PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE: &str =
    "layouts/private_callback_slot_demand_compile";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE];
