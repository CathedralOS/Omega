/// Source-span-free compiler-internal custody for one direct concrete placed-
/// view machine input. The complete validated placement is retained because
/// its compact compatibility fingerprint is not semantic authority. Symbol
/// handles remain private joins and never cross the Terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPlacedViewInput {
    pub machine: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub position: u32,
    pub parameter: symbols::SymbolHandle,
    pub reference_access: language_core::ReferenceAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view: symbols::SymbolHandle,
    pub policy: symbols::SymbolHandle,
    pub policy_plan_machine: symbols::SymbolHandle,
    pub schema: symbols::SymbolHandle,
    pub placement: access_plans::ValidatedPlacementPlan,
}
