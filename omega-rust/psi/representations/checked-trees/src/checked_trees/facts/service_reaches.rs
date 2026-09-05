//! EFX: normalized boundary-service reach facts. These rows are built from
//! resolved boundary-trait identities and recursive call-graph propagation;
//! no legacy effect name or numeric bit participates. Machine, state, and
//! call summaries use grouped arenas plus shared row identities, avoiding
//! per-node service vectors and preserving call-site joins downstream.

use arena::{Arena, HandleSpan};
use language_semantics::{ServiceReachRowId, ServiceReachRowTable, ServiceReachTable};
use symbols::SymbolHandle;

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

    /// Project one machine's complete normalized service contract from its
    /// independently published reach facts. Public empty and private empty
    /// remain distinct through the retained interface discriminator.
    pub fn plan_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<language_semantics::ServiceReachPlan> {
        self.for_machine(machine)
            .map(|fact| language_semantics::ServiceReachPlan {
                interface: fact.interface,
                checked_inferred: fact.inferred_transitive,
            })
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
    pub interface: language_semantics::ServiceReachInterface,
    pub published_ceiling: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    /// Exact checked-body reach before unresolved installation-selected upper
    /// bounds are admitted.
    pub concrete_transitive: ServiceReachRowId,
    pub effective: ServiceReachRowId,
    pub concrete_effective: ServiceReachRowId,
    /// Exact bounded requirement rows that final composition must resolve.
    /// The ordinary reach rows retain their conservative upper bounds.
    pub unresolved_installation_reaches: Vec<flow_effects::InstallationReachRequirement>,
    pub states: HandleSpan<StateServiceReachRows>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateServiceReachRows {
    pub state: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<flow_effects::InstallationReachRequirement>,
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
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<flow_effects::InstallationReachRequirement>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_projection_preserves_public_empty_and_missing_distinctions() {
        let published = SymbolHandle::from_arena_index(1);
        let internal = SymbolHandle::from_arena_index(2);
        let unknown = SymbolHandle::from_arena_index(3);
        let mut facts = ServiceReachFacts::default();
        facts.machines.append_to_span(
            &mut facts.root_machines,
            MachineServiceReachRows {
                machine: published,
                interface: language_semantics::ServiceReachInterface::PublishedCeiling(
                    ServiceReachRowTable::EMPTY_ROW,
                ),
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                ..Default::default()
            },
        );
        facts.machines.append_to_span(
            &mut facts.root_machines,
            MachineServiceReachRows {
                machine: internal,
                interface: language_semantics::ServiceReachInterface::InternalInferred,
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                ..Default::default()
            },
        );

        assert_eq!(
            facts.plan_for_machine(published),
            Some(language_semantics::ServiceReachPlan {
                interface: language_semantics::ServiceReachInterface::PublishedCeiling(
                    ServiceReachRowTable::EMPTY_ROW,
                ),
                checked_inferred: ServiceReachRowTable::EMPTY_ROW,
            })
        );
        assert_eq!(
            facts.plan_for_machine(internal),
            Some(language_semantics::ServiceReachPlan {
                interface: language_semantics::ServiceReachInterface::InternalInferred,
                checked_inferred: ServiceReachRowTable::EMPTY_ROW,
            })
        );
        assert_eq!(facts.plan_for_machine(unknown), None);
    }
}
