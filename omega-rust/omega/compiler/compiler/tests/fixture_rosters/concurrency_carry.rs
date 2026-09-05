//! The dedicated concurrency carry test executes these exact checked-stage rows.
//! Corpus inventory shares them without scheduling a native scheduler test.

pub(crate) const PASS_CANARIES: &[&str] = &[
    "concurrency/suspend_after_last_use_compile",
    "concurrency/suspend_after_earlier_operand_compile",
    "concurrency/suspend_after_self_field_last_use_compile",
];

pub(crate) const FAIL_CANARIES: &[(&str, &str)] = &[
    (
        "concurrency/suspend_live_value_rejected",
        "may suspend while `message` remains live",
    ),
    (
        "concurrency/suspend_self_field_reachable_state_rejected",
        "may suspend while `self.message` remains live",
    ),
    (
        "concurrency/suspend_call_argument_rejected",
        "may suspend while `message` remains live",
    ),
    (
        "concurrency/suspend_later_operand_rejected",
        "nested inside a partially evaluated expression",
    ),
];
