use psi_effects::{
    CallOperational, MachineOperational, OperationalPlan, ServiceReachInferencePlan,
    StateOperational,
};
use psi_language_semantics::{
    BlockingSummary, ServiceReachRowTable, ServiceReachSummary, SuspensionSummary,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

/// The typed-tree report's normalized behavior view. Service identities and
/// operational possibilities stay independent; the legacy flat effect set is
/// neither exposed nor consulted by report rendering.
pub(super) struct TypedBehaviorPlan {
    operational: OperationalPlan,
    service_reaches: ServiceReachInferencePlan,
}

impl TypedBehaviorPlan {
    pub(super) fn infer(program: &TypedTrees) -> Self {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        Self {
            operational,
            service_reaches,
        }
    }

    pub(super) fn service_rows(&self) -> &ServiceReachRowTable {
        &self.service_reaches.rows
    }

    pub(super) fn machine(
        &self,
        symbol: SymbolHandle,
    ) -> (ServiceReachSummary, SuspensionSummary, BlockingSummary) {
        let service_reach = self
            .service_reaches
            .for_machine(symbol)
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();
        let (suspension, blocking) = self
            .machine_operations(symbol)
            .map(|summary| {
                (
                    self.machine_suspension(summary),
                    self.machine_blocking(summary),
                )
            })
            .unwrap_or_default();
        (service_reach, suspension, blocking)
    }

    pub(super) fn state(
        &self,
        symbol: SymbolHandle,
    ) -> (ServiceReachSummary, SuspensionSummary, BlockingSummary) {
        let service_reach = self
            .service_reaches
            .for_state(symbol)
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();
        let (suspension, blocking) = self
            .state_operations(symbol)
            .map(|summary| (self.state_suspension(summary), self.state_blocking(summary)))
            .unwrap_or_default();
        (service_reach, suspension, blocking)
    }

    pub(super) fn call(
        &self,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) -> (ServiceReachSummary, SuspensionSummary, BlockingSummary) {
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
        let (suspension, blocking) = self
            .call_operations(state_symbol, statement_index, target_symbol)
            .map(call_summaries)
            .unwrap_or_default();
        (service_reach, suspension, blocking)
    }

    fn machine_operations(&self, symbol: SymbolHandle) -> Option<&MachineOperational> {
        self.operational
            .machines()
            .iter()
            .find(|summary| summary.symbol == symbol)
    }

    fn state_operations(&self, symbol: SymbolHandle) -> Option<&StateOperational> {
        self.operational
            .machines()
            .iter()
            .flat_map(|machine| self.operational.states.span_or_empty(machine.states))
            .find(|summary| summary.symbol == symbol)
    }

    fn call_operations(
        &self,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) -> Option<&CallOperational> {
        let state = self.state_operations(state_symbol)?;
        self.operational
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|summary| {
                summary.statement_index == statement_index
                    && summary.target_state_symbol == target_symbol
            })
    }

    fn machine_suspension(&self, machine: &MachineOperational) -> SuspensionSummary {
        let mut direct_may_suspend = false;
        for state in self.operational.states.span_or_empty(machine.states) {
            direct_may_suspend |= self.state_direct_may_suspend(state);
        }
        SuspensionSummary {
            direct_may_suspend,
            transitive_may_suspend: machine.transitive_may_suspend,
        }
    }

    fn machine_blocking(&self, machine: &MachineOperational) -> BlockingSummary {
        let mut direct_may_block = false;
        for state in self.operational.states.span_or_empty(machine.states) {
            direct_may_block |= self.state_direct_may_block(state);
        }
        BlockingSummary {
            direct_may_block,
            transitive_may_block: machine.transitive_may_block,
        }
    }

    fn state_suspension(&self, state: &StateOperational) -> SuspensionSummary {
        SuspensionSummary {
            direct_may_suspend: self.state_direct_may_suspend(state),
            transitive_may_suspend: state.transitive_may_suspend,
        }
    }

    fn state_blocking(&self, state: &StateOperational) -> BlockingSummary {
        BlockingSummary {
            direct_may_block: self.state_direct_may_block(state),
            transitive_may_block: state.transitive_may_block,
        }
    }

    fn state_direct_may_suspend(&self, state: &StateOperational) -> bool {
        let mut direct_may_suspend = state.direct_may_suspend;
        for call in self.operational.calls.span_or_empty(state.calls) {
            direct_may_suspend |= call.direct_may_suspend;
        }
        direct_may_suspend
    }

    fn state_direct_may_block(&self, state: &StateOperational) -> bool {
        let mut direct_may_block = state.direct_may_block;
        for call in self.operational.calls.span_or_empty(state.calls) {
            direct_may_block |= call.direct_may_block;
        }
        direct_may_block
    }
}

fn call_summaries(call: &CallOperational) -> (SuspensionSummary, BlockingSummary) {
    (
        SuspensionSummary {
            direct_may_suspend: call.direct_may_suspend,
            transitive_may_suspend: call.transitive_may_suspend,
        },
        BlockingSummary {
            direct_may_block: call.direct_may_block,
            transitive_may_block: call.transitive_may_block,
        },
    )
}
