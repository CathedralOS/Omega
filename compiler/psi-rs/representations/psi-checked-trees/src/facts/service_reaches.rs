//! EFX: normalized boundary-service reach facts. These rows are built from
//! resolved boundary-trait identities and recursive call-graph propagation;
//! no legacy effect name or numeric bit participates. Machine, state, and
//! call summaries use grouped arenas plus shared row identities, avoiding
//! per-node service vectors and preserving call-site joins downstream.

use psi_arena::{Arena, HandleSpan};
use psi_language_semantics::{ServiceReachRowId, ServiceReachRowTable, ServiceReachTable};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachFacts {
    pub services: ServiceReachTable,
    pub rows: ServiceReachRowTable,
    pub root_machines: HandleSpan<MachineServiceReachRows>,
    pub machines: Arena<MachineServiceReachRows>,
    pub states: Arena<StateServiceReachRows>,
    pub calls: Arena<CallServiceReachRows>,
}

impl ServiceReachFacts {
    pub fn machines(&self) -> &[MachineServiceReachRows] {
        self.machines.span_or_empty(self.root_machines)
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachRows> {
        self.machines().iter().find(|fact| fact.machine == machine)
    }

    pub fn states_for(&self, machine: &MachineServiceReachRows) -> &[StateServiceReachRows] {
        self.states.span_or_empty(machine.states)
    }

    pub fn for_state(&self, state: SymbolHandle) -> Option<&StateServiceReachRows> {
        self.states
            .iter()
            .map(|(_, fact)| fact)
            .find(|fact| fact.state == state)
    }

    pub fn calls_for(&self, state: &StateServiceReachRows) -> &[CallServiceReachRows] {
        self.calls.span_or_empty(state.calls)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineServiceReachRows {
    pub machine: SymbolHandle,
    /// Exact public/private interface from symbol-resolved inference. This is
    /// not reconstructed from `published_ceiling`, because a public empty
    /// ceiling and private empty inference share the same row contents.
    pub interface: psi_language_semantics::ServiceReachInterface,
    pub published_ceiling: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub effective: ServiceReachRowId,
    pub states: HandleSpan<StateServiceReachRows>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateServiceReachRows {
    pub state: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub calls: HandleSpan<CallServiceReachRows>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallServiceReachRows {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_state: SymbolHandle,
    pub target_machine: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
}
