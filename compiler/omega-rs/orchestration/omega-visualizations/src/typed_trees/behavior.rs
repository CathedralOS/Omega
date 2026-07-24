use omega_core::semantics::{OperationalMaySummary, ServiceReachRowTable, ServiceReachSummary};
use omega_core::symbols::SymbolHandle;
use omega_effects::{
    CallEffects, EffectPlan, MachineEffects, ServiceReachInferencePlan, StateEffects,
};
use omega_typed_trees::TypedTrees;

/// The typed-tree report's normalized behavior view. Service identities and
/// operational possibilities stay independent; the legacy flat effect set is
/// neither exposed nor consulted by report rendering.
pub(super) struct TypedBehaviorPlan {
    effects: EffectPlan,
    service_reaches: ServiceReachInferencePlan,
}

impl TypedBehaviorPlan {
    pub(super) fn infer(program: &TypedTrees) -> Self {
        let effects = omega_effects::infer_effects(program);
        let service_reaches = omega_effects::infer_service_reaches(program, &effects);
        Self {
            effects,
            service_reaches,
        }
    }

    pub(super) fn service_rows(&self) -> &ServiceReachRowTable {
        &self.service_reaches.rows
    }

    pub(super) fn machine(
        &self,
        symbol: SymbolHandle,
    ) -> (ServiceReachSummary, OperationalMaySummary) {
        let service_reach = self
            .service_reaches
            .for_machine(symbol)
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();
        let operational = self
            .machine_effects(symbol)
            .map(|summary| self.machine_operational(summary))
            .unwrap_or_default();
        (service_reach, operational)
    }

    pub(super) fn state(
        &self,
        symbol: SymbolHandle,
    ) -> (ServiceReachSummary, OperationalMaySummary) {
        let service_reach = self
            .service_reaches
            .for_state(symbol)
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();
        let operational = self
            .state_effects(symbol)
            .map(|summary| self.state_operational(summary))
            .unwrap_or_default();
        (service_reach, operational)
    }

    pub(super) fn call(
        &self,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) -> (ServiceReachSummary, OperationalMaySummary) {
        let service_reach = self
            .service_reaches
            .for_state(state_symbol)
            .and_then(|state| {
                self.service_reaches
                    .calls_for(state)
                    .iter()
                    .find(|summary| {
                        summary.statement_index == statement_index
                            && summary.target_state == target_symbol
                    })
            })
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();
        let operational = self
            .call_effects(state_symbol, statement_index, target_symbol)
            .map(call_operational)
            .unwrap_or_default();
        (service_reach, operational)
    }

    fn machine_effects(&self, symbol: SymbolHandle) -> Option<&MachineEffects> {
        self.effects
            .machines()
            .iter()
            .find(|summary| summary.symbol == symbol)
    }

    fn state_effects(&self, symbol: SymbolHandle) -> Option<&StateEffects> {
        self.effects
            .machines()
            .iter()
            .flat_map(|machine| self.effects.states.span_or_empty(machine.states))
            .find(|summary| summary.symbol == symbol)
    }

    fn call_effects(
        &self,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) -> Option<&CallEffects> {
        let state = self.state_effects(state_symbol)?;
        self.effects
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|summary| {
                summary.statement_index == statement_index
                    && summary.target_state_symbol == target_symbol
            })
    }

    fn machine_operational(&self, machine: &MachineEffects) -> OperationalMaySummary {
        let mut direct_may_suspend = false;
        let mut direct_may_block = false;
        for state in self.effects.states.span_or_empty(machine.states) {
            let direct = self.state_direct_operational(state);
            direct_may_suspend |= direct.0;
            direct_may_block |= direct.1;
        }
        OperationalMaySummary {
            direct_may_suspend,
            transitive_may_suspend: machine.transitive_may_suspend,
            direct_may_block,
            transitive_may_block: machine.transitive_may_block,
        }
    }

    fn state_operational(&self, state: &StateEffects) -> OperationalMaySummary {
        let (direct_may_suspend, direct_may_block) = self.state_direct_operational(state);
        OperationalMaySummary {
            direct_may_suspend,
            transitive_may_suspend: state.transitive_may_suspend,
            direct_may_block,
            transitive_may_block: state.transitive_may_block,
        }
    }

    fn state_direct_operational(&self, state: &StateEffects) -> (bool, bool) {
        let mut direct_may_suspend = state.direct_may_suspend;
        let mut direct_may_block = state.direct_may_block;
        for call in self.effects.calls.span_or_empty(state.calls) {
            direct_may_suspend |= call.direct_may_suspend;
            direct_may_block |= call.direct_may_block;
        }
        (direct_may_suspend, direct_may_block)
    }
}

fn call_operational(call: &CallEffects) -> OperationalMaySummary {
    OperationalMaySummary {
        direct_may_suspend: call.direct_may_suspend,
        transitive_may_suspend: call.transitive_may_suspend,
        direct_may_block: call.direct_may_block,
        transitive_may_block: call.transitive_may_block,
    }
}
