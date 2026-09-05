/// Source-span-free compiler-internal custody for one direct concrete placed-
/// view machine input. The complete validated placement is retained because
/// its compact compatibility fingerprint is not semantic authority. Symbol
/// handles remain private joins and never cross the Terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPlacedViewInput {
    pub machine: psi_symbols::SymbolHandle,
    pub state: psi_symbols::SymbolHandle,
    pub position: u32,
    pub parameter: psi_symbols::SymbolHandle,
    pub reference_access: psi_language_core::ReferenceAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view: psi_symbols::SymbolHandle,
    pub policy: psi_symbols::SymbolHandle,
    pub policy_plan_machine: psi_symbols::SymbolHandle,
    pub schema: psi_symbols::SymbolHandle,
    pub placement: psi_access_plans::ValidatedPlacementPlan,
}
