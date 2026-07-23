//! EFX: normalized boundary-service reach facts. These rows are built from
//! resolved boundary-trait identities and recursive call-graph propagation;
//! no legacy effect name or numeric bit participates.

use omega_core::semantics::{ServiceReachRowId, ServiceReachRowTable, ServiceReachTable};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachFacts {
    pub services: ServiceReachTable,
    pub rows: ServiceReachRowTable,
    pub machines: Vec<MachineServiceReachRows>,
}

impl ServiceReachFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachRows> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineServiceReachRows {
    pub machine: SymbolHandle,
    pub published_ceiling: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
}
