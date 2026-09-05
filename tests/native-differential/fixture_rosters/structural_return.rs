//! Exact corpus inputs consumed by the structural return test owner.
//! Compilation phases, targets, and differential assertions remain in the tests.

pub(crate) const STRUCTURAL_CONTENT_PASSTHROUGH: &str =
    "terminal_psi/structural_content_passthrough";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[STRUCTURAL_CONTENT_PASSTHROUGH];
