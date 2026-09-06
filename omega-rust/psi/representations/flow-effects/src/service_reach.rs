use arena::{Arena, HandleSpan};
use language_semantics::{
    ServiceReachId, ServiceReachInterface, ServiceReachRowId, ServiceReachRowTable,
};
use symbols::SymbolHandle;

/// The symbol-resolved recursive service summary for one machine. All sets
/// are interned in the plan's shared row table; child state/call summaries are
/// grouped in arenas instead of allocating one small `Vec` per parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineServiceReachInference {
    pub machine: SymbolHandle,
    /// Exact public/private contract axis. A published empty ceiling remains
    /// distinct from an internal empty inference.
    pub interface: ServiceReachInterface,
    pub published: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    /// Exact checked-body reach before unresolved installation-selected upper
    /// bounds are admitted. This remains independent of the modular
    /// `concrete_effective` row consumed by callers.
    pub concrete_transitive: ServiceReachRowId,
    /// Reach not contributed solely by an installation-selected upper bound.
    /// Final composition unions selected rows into this base; it never tries
    /// to subtract upper bounds from the flattened conservative set.
    pub concrete_effective: ServiceReachRowId,
    /// Exact installation-selected requirement rows reachable from this
    /// machine. Their upper bounds remain in the ordinary service rows for
    /// conservative preselection auditing; composition must later substitute
    /// one selected provider row for every entry here.
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    /// The modular summary callers consume: published for a pinned/authored
    /// interface, inferred for a private checked body.
    pub effective: ServiceReachRowId,
    pub states: HandleSpan<StateServiceReachInference>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateServiceReachInference {
    pub state: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    pub calls: HandleSpan<CallServiceReachInference>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallServiceReachInference {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_state: SymbolHandle,
    pub target_machine: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallationReachRequirement {
    /// Exact normalized boundary-trait requirement identity.
    pub requirement: SymbolHandle,
    /// Conservative upper bound published by `reaches <= Bound`.
    pub upper_bound: ServiceReachRowId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachInferencePlan {
    pub rows: ServiceReachRowTable,
    pub root_machines: HandleSpan<MachineServiceReachInference>,
    pub machines: Arena<MachineServiceReachInference>,
    pub states: Arena<StateServiceReachInference>,
    pub calls: Arena<CallServiceReachInference>,
}

impl ServiceReachInferencePlan {
    pub fn machines(&self) -> &[MachineServiceReachInference] {
        self.machines.span_or_empty(self.root_machines)
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachInference> {
        self.machines()
            .iter()
            .find(|summary| summary.machine == machine)
    }

    pub fn states_for(
        &self,
        machine: &MachineServiceReachInference,
    ) -> &[StateServiceReachInference] {
        self.states.span_or_empty(machine.states)
    }

    pub fn for_state(&self, state: SymbolHandle) -> Option<&StateServiceReachInference> {
        self.states
            .iter()
            .map(|(_, summary)| summary)
            .find(|summary| summary.state == state)
    }

    pub fn calls_for(&self, state: &StateServiceReachInference) -> &[CallServiceReachInference] {
        self.calls.span_or_empty(state.calls)
    }

    pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
        self.rows.services(row)
    }
}
