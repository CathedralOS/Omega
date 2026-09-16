//! Corpus sources used by package artifact and provider-custody tests.
//! Missing-custody rejection is a test scenario, not a corpus bucket change.

pub(crate) const NO_SELECTION_EMPTY_ENTRY: &str = "optimizer/no_selection_empty_entry";
pub(crate) const ASM_PORT_OUT_FINAL_VALIDATION: &str = "inline_asm/asm_port_out_final_validation";

// `depend_as` package sources below are exercised through the module-indices
// scenarios that synthesize the same graphs; the reviewed-fixture route only
// synthesizes inputs for ordinary-std dependencies, so these stay inventory.
pub(crate) const MODULES_PACKAGE_BARE_CASES: &str = "modules/package_bare_cases";
pub(crate) const MODULES_QUALIFIED_CASE_MEMBERSHIP: &str = "modules/qualified_case_membership";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    NO_SELECTION_EMPTY_ENTRY,
    ASM_PORT_OUT_FINAL_VALIDATION,
    MODULES_PACKAGE_BARE_CASES,
    MODULES_QUALIFIED_CASE_MEMBERSHIP,
];
