//! Corpus inputs used by the dedicated no-selection compatibility tests.
//! Native goldens and per-target diagnostic assertions remain with that owner.

pub(crate) const RUNTIME_WRITE_NO_NEWLINE_EXIT: &str = "host/runtime_write_no_newline_exit";
pub(crate) const NO_SELECTION_EMPTY_ENTRY: &str = "optimizer/no_selection_empty_entry";
pub(crate) const ROLLBACK_TO_NO_SELECTION_EMPTY_ENTRY: &str =
    "optimizer/rollback_to_no_selection_empty_entry";
pub(crate) const NO_SELECTION_WRONG_ARITY: &str = "optimizer/no_selection_wrong_arity";

// The dedicated target uses named cases; corpus inventory consumes this slice.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_WRITE_NO_NEWLINE_EXIT,
    NO_SELECTION_EMPTY_ENTRY,
    ROLLBACK_TO_NO_SELECTION_EMPTY_ENTRY,
];

// The failure owner reads expected.txt and compares the complete diagnostics.
#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const FILE_EXPECTATION_FAIL_CANARIES: &[&str] = &[NO_SELECTION_WRONG_ARITY];
