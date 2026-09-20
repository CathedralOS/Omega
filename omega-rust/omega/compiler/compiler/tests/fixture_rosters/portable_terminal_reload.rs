//! Portable Terminal serialization uses the same source as the inventory.

pub const SELECTED_EMPTY_COMPONENT: &str = "terminal_psi/selected_empty_component";

/// Contract-bearing canary: a requires-guarded call, an ensures clause, and
/// an exact-subtraction obligation must all be reconstructed and discharged
/// by the consumer after producer state is gone.
pub const PORTABLE_CONTRACT_COMPONENT: &str = "terminal_psi/portable_contract_component";

pub const PASS_CANARIES: &[&str] = &[SELECTED_EMPTY_COMPONENT, PORTABLE_CONTRACT_COMPONENT];

/// Fixtures the serialized reload control runs. Membership requires a
/// standalone Terminal product and an interpretable entry.
pub const RELOAD_CANARIES: &[&str] = &[SELECTED_EMPTY_COMPONENT, PORTABLE_CONTRACT_COMPONENT];

/// Reload fixtures whose programs carry real contract/operation
/// obligations; consume asserts the independently reconstructed obligation
/// ledger is non-empty for these.
pub const OBLIGATION_BEARING_RELOAD_CANARIES: &[&str] = &[PORTABLE_CONTRACT_COMPONENT];
