//! The corpus source inspected by call-acknowledgement tests.
//! Inline source probes and semantic assertions stay with that test target.

pub(crate) const TASK_RUNTIME_MACHINE_SELECTION_COMPILE: &str =
    "tasks/task_runtime_machine_selection_compile";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[TASK_RUNTIME_MACHINE_SELECTION_COMPILE];
