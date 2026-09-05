//! Corpus sources used by package artifact and provider-custody tests.
//! Missing-custody rejection is a test scenario, not a corpus bucket change.

pub(crate) const NO_SELECTION_EMPTY_ENTRY: &str = "optimizer/no_selection_empty_entry";
pub(crate) const ASM_PORT_OUT_FINAL_VALIDATION: &str = "inline_asm/asm_port_out_final_validation";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] =
    &[NO_SELECTION_EMPTY_ENTRY, ASM_PORT_OUT_FINAL_VALIDATION];
