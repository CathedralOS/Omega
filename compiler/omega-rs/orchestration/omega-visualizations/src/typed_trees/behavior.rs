use omega_core::semantics::{OperationalMaySummary, ServiceReachRowTable, ServiceReachSummary};
use omega_core::symbols::SymbolHandle;
use omega_effects::{
    CallOperational, MachineOperational, OperationalPlan, ServiceReachInferencePlan,
    StateOperational,
};
use omega_typed_trees::TypedTrees;

/// The typed-tree report's normalized behavior view. Service identities and
/// operational possibilities stay independent; the legacy flat effect set is
/// neither exposed nor consulted by report rendering.
pub(super) struct TypedBehaviorPlan {
    operations: OperationalPlan,
    service_reaches: ServiceReachInferencePlan,
}

impl TypedBehaviorPlan {
    pub(super) fn infer(program: &TypedTrees) -> Self {
        let operations = omega_effects::infer_operational_may(program);
        let service_reaches = omega_effects::infer_service_reaches(program, &operations);
        Self {
            operations,
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
            .machine_operations(symbol)
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
            .state_operations(symbol)
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
            .call_operations(state_symbol, statement_index, target_symbol)
            .map(call_operational)
            .unwrap_or_default();
        (service_reach, operational)
    }

    fn machine_operations(&self, symbol: SymbolHandle) -> Option<&MachineOperational> {
        self.operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == symbol)
    }

    fn state_operations(&self, symbol: SymbolHandle) -> Option<&StateOperational> {
        self.operations
            .machines()
            .iter()
            .flat_map(|machine| self.operations.states.span_or_empty(machine.states))
            .find(|summary| summary.symbol == symbol)
    }

    fn call_operations(
        &self,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) -> Option<&CallOperational> {
        let state = self.state_operations(state_symbol)?;
        self.operations
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|summary| {
                summary.statement_index == statement_index
                    && summary.target_state_symbol == target_symbol
            })
    }

    fn machine_operational(&self, machine: &MachineOperational) -> OperationalMaySummary {
        let mut direct_may_suspend = false;
        let mut direct_may_block = false;
        for state in self.operations.states.span_or_empty(machine.states) {
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

    fn state_operational(&self, state: &StateOperational) -> OperationalMaySummary {
        let (direct_may_suspend, direct_may_block) = self.state_direct_operational(state);
        OperationalMaySummary {
            direct_may_suspend,
            transitive_may_suspend: state.transitive_may_suspend,
            direct_may_block,
            transitive_may_block: state.transitive_may_block,
        }
    }

    fn state_direct_operational(&self, state: &StateOperational) -> (bool, bool) {
        let mut direct_may_suspend = state.direct_may_suspend;
        let mut direct_may_block = state.direct_may_block;
        for call in self.operations.calls.span_or_empty(state.calls) {
            direct_may_suspend |= call.direct_may_suspend;
            direct_may_block |= call.direct_may_block;
        }
        (direct_may_suspend, direct_may_block)
    }
}

fn call_operational(call: &CallOperational) -> OperationalMaySummary {
    OperationalMaySummary {
        direct_may_suspend: call.direct_may_suspend,
        transitive_may_suspend: call.transitive_may_suspend,
        direct_may_block: call.direct_may_block,
        transitive_may_block: call.transitive_may_block,
    }
}
