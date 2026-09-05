//! Corpus inputs used by target-specific build admission tests.
//! Each target's positive or negative assertion remains with its test owner.

pub(crate) const NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT: &str =
    "float/named_provider_fused_multiply_add_exit";
pub(crate) const X86_FMA_PLAN_ASSOCIATION: &str = "float/x86_fma_plan_association";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT,
    X86_FMA_PLAN_ASSOCIATION,
];
