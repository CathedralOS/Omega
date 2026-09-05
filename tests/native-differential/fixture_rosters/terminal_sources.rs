//! Exact corpus inputs consumed by the terminal sources test owner.
//! Compilation phases, targets, and differential assertions remain in the tests.

#[path = "integer_control.rs"]
mod integer_control;
pub(crate) use integer_control::INTEGER_CONTROL_CONTRACT;

pub(crate) const SELECTED_EMPTY_COMPONENT: &str = "terminal_psi/selected_empty_component";
pub(crate) const SELECTED_OPTIMIZER_COMPONENT: &str = "terminal_psi/selected_optimizer_component";
pub(crate) const SELECTED_LOWERING_OPTIMIZER_COMPONENT: &str =
    "terminal_psi/selected_lowering_optimizer_component";
pub(crate) const UNSUPPORTED_OPTIMIZER_COMPONENT: &str =
    "terminal_psi/unsupported_optimizer_component";
pub(crate) const STRUCTURAL_SCALAR_TRAIT_OPERATOR: &str =
    "terminal_psi/structural_scalar_trait_operator";
pub(crate) const MEMBER_CRASH_CONTRACT_BOUNDARY: &str =
    "terminal_psi/member_crash_contract_boundary";
pub(crate) const PROVIDER_RECEIVER_PROGRESS_INSTALLATION: &str =
    "progress/provider_receiver_progress_installation";

#[allow(
    dead_code,
    reason = "inventory entrypoint shared with the dedicated test target"
)]
pub(crate) const PASS_CANARIES: &[&str] = &[
    INTEGER_CONTROL_CONTRACT,
    SELECTED_EMPTY_COMPONENT,
    SELECTED_OPTIMIZER_COMPONENT,
    SELECTED_LOWERING_OPTIMIZER_COMPONENT,
    UNSUPPORTED_OPTIMIZER_COMPONENT,
    STRUCTURAL_SCALAR_TRAIT_OPERATOR,
    MEMBER_CRASH_CONTRACT_BOUNDARY,
    PROVIDER_RECEIVER_PROGRESS_INSTALLATION,
];
