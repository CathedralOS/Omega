//! Exact corpus inputs consumed by the content custody test owner.
//! Compilation phases, targets, and differential assertions remain in the tests.

pub(crate) const CONTENT_CUSTODY_EXIT: &str = "terminal_psi/content_custody_exit";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[CONTENT_CUSTODY_EXIT];
