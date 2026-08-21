use omega_calling_conventions::BoundaryEntryPlan;
use omega_control_flow::StateKey;
use psi_checked_trees::NominalMachineUseSite;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

/// Target-owned callback recipe joined to one admitted nominal machine use.
///
/// The checked program owns the semantic admission and only retains the
/// evaluated plan fingerprint. This row carries the exact validated plan past
/// orchestration so thunk lowering never has to rediscover ABI placement from
/// names, types, or a convention oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundNominalCallbackPlacement {
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    pub boundary_calling_plan_fingerprint: u64,
    pub boundary_entry_plan: BoundaryEntryPlan,
}

/// One private inbound function that later target lowering must emit.
///
/// `placement_index` joins back to the exact validated placement row without
/// cloning it. The symbol is compiler-private planned object identity, never
/// an Omega value or a source-level address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackThunkPlan {
    pub placement_index: usize,
    pub entry_key: StateKey,
    pub private_symbol: Arc<str>,
}
