//! Corpus inputs for checked policy vocabulary and source access-plan evaluation.

pub(crate) const ACCESS_PLAN_INACCESSIBLE_SEED: &str = "layouts/access_plan_inaccessible_seed";
pub(crate) const PLACED_POLICY_CORE_RECORDS: &str = "layouts/placed_policy_core_records";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] =
    &[ACCESS_PLAN_INACCESSIBLE_SEED, PLACED_POLICY_CORE_RECORDS];
