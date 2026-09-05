//! NOT COMPILED - no `mod typed_trees` declaration exists anywhere in this crate,
//! so rustc never sees this file. See the @Cleanup in lib.rs.

use flow_effects::{OperationalPlan, ServiceReachInferencePlan};
use language_semantics::{
    BlockingSummary, ServiceReachRowTable, ServiceReachSummary, SuspensionSummary,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// The typed-tree report's normalized behavior view. Service identities,
/// suspension, and blocking stay independent; the transient operational
/// inference carrier is projected and discarded before report rendering.
pub(super) struct TypedBehaviorPlan {
    service_reaches: ServiceReachInferencePlan,
    machines: Vec<MachineBehavior>,
    states: Vec<StateBehavior>,
    calls: Vec<CallBehavior>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MachineBehavior {
    symbol: SymbolHandle,
    suspension: SuspensionSummary,
    blocking: BlockingSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StateBehavior {
    symbol: SymbolHandle,
    suspension: SuspensionSummary,
    blocking: BlockingSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallBehavior {
    state_symbol: SymbolHandle,
    statement_index: usize,
    target_symbol: SymbolHandle,
    suspension: SuspensionSummary,
    blocking: BlockingSummary,
}

impl TypedBehaviorPlan {
    pub(super) fn infer(program: &TypedTrees) -> Self {
        let operational = flow_effects::infer_operational_may(program);
        let service_reaches = flow_effects::infer_service_reaches(program, &operational);
        let (machines, states, calls) = project_operational_axes(&operational);
        Self {
            service_reaches,
            machines,
            states,
            calls,
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
            .machines
            .iter()
            .find(|summary| summary.symbol == symbol)
            .map(|summary| (summary.suspension, summary.blocking))
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
            .states
            .iter()
            .find(|summary| summary.symbol == symbol)
            .map(|summary| (summary.suspension, summary.blocking))
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
            .calls
            .iter()
            .find(|summary| {
                summary.state_symbol == state_symbol
                    && summary.statement_index == statement_index
                    && summary.target_symbol == target_symbol
            })
            .map(|summary| (summary.suspension, summary.blocking))
            .unwrap_or_default();
        (service_reach, suspension, blocking)
    }
}

fn project_operational_axes(
    operational: &OperationalPlan,
) -> (Vec<MachineBehavior>, Vec<StateBehavior>, Vec<CallBehavior>) {
    let mut machines = Vec::new();
    let mut states = Vec::new();
    let mut calls = Vec::new();

    for machine in operational.machines() {
        let machine_states = operational.states.span_or_empty(machine.states);
        let direct_may_suspend = machine_states
            .iter()
            .any(|state| state_direct_may_suspend(operational, state));
        let direct_may_block = machine_states
            .iter()
            .any(|state| state_direct_may_block(operational, state));
        machines.push(MachineBehavior {
            symbol: machine.symbol,
            suspension: SuspensionSummary {
                direct_may_suspend,
                transitive_may_suspend: machine.transitive_may_suspend,
            },
            blocking: BlockingSummary {
                direct_may_block,
                transitive_may_block: machine.transitive_may_block,
            },
        });

        for state in machine_states {
            states.push(StateBehavior {
                symbol: state.symbol,
                suspension: SuspensionSummary {
                    direct_may_suspend: state_direct_may_suspend(operational, state),
                    transitive_may_suspend: state.transitive_may_suspend,
                },
                blocking: BlockingSummary {
                    direct_may_block: state_direct_may_block(operational, state),
                    transitive_may_block: state.transitive_may_block,
                },
            });
            for call in operational.calls.span_or_empty(state.calls) {
                calls.push(CallBehavior {
                    state_symbol: state.symbol,
                    statement_index: call.statement_index,
                    target_symbol: call.target_state_symbol,
                    suspension: SuspensionSummary {
                        direct_may_suspend: call.direct_may_suspend,
                        transitive_may_suspend: call.transitive_may_suspend,
                    },
                    blocking: BlockingSummary {
                        direct_may_block: call.direct_may_block,
                        transitive_may_block: call.transitive_may_block,
                    },
                });
            }
        }
    }

    (machines, states, calls)
}

fn state_direct_may_suspend(
    operational: &OperationalPlan,
    state: &flow_effects::StateOperational,
) -> bool {
    state.direct_may_suspend
        || operational
            .calls
            .span_or_empty(state.calls)
            .iter()
            .any(|call| call.direct_may_suspend)
}

fn state_direct_may_block(
    operational: &OperationalPlan,
    state: &flow_effects::StateOperational,
) -> bool {
    state.direct_may_block
        || operational
            .calls
            .span_or_empty(state.calls)
            .iter()
            .any(|call| call.direct_may_block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use flow_effects::{CallOperational, MachineOperational, StateOperational};

    fn operational_fixture() -> (OperationalPlan, SymbolHandle, SymbolHandle, SymbolHandle) {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let target_symbol = SymbolHandle::from_arena_index(3);
        let mut operational = OperationalPlan::default();
        let mut calls = HandleSpan::empty();
        operational.calls.append_to_span(
            &mut calls,
            CallOperational {
                statement_index: 7,
                target_state_symbol: target_symbol,
                direct_may_suspend: true,
                transitive_may_suspend: true,
                direct_may_block: false,
                transitive_may_block: false,
                ..Default::default()
            },
        );
        let mut states = HandleSpan::empty();
        operational.states.append_to_span(
            &mut states,
            StateOperational {
                symbol: state_symbol,
                direct_may_block: true,
                transitive_may_suspend: true,
                transitive_may_block: false,
                calls,
                ..Default::default()
            },
        );
        operational.machines.append_to_span(
            &mut operational.root_machines,
            MachineOperational {
                symbol: machine_symbol,
                transitive_may_suspend: true,
                transitive_may_block: false,
                states,
                ..Default::default()
            },
        );
        (operational, machine_symbol, state_symbol, target_symbol)
    }

    #[test]
    fn projection_keeps_suspension_and_blocking_orthogonal() {
        let (operational, machine_symbol, state_symbol, target_symbol) = operational_fixture();
        let (machines, states, calls) = project_operational_axes(&operational);

        assert_eq!(
            machines[0],
            MachineBehavior {
                symbol: machine_symbol,
                suspension: SuspensionSummary {
                    direct_may_suspend: true,
                    transitive_may_suspend: true,
                },
                blocking: BlockingSummary {
                    direct_may_block: true,
                    transitive_may_block: false,
                },
            }
        );
        assert_eq!(states[0].symbol, state_symbol);
        assert!(states[0].suspension.direct_may_suspend);
        assert!(states[0].blocking.direct_may_block);
        assert_eq!(calls[0].target_symbol, target_symbol);
        assert!(calls[0].suspension.transitive_may_suspend);
        assert!(!calls[0].blocking.transitive_may_block);
    }

    #[test]
    fn retained_axes_default_for_unknown_exact_coordinates() {
        let (operational, _, _, _) = operational_fixture();
        let (machines, states, calls) = project_operational_axes(&operational);
        let behavior = TypedBehaviorPlan {
            service_reaches: ServiceReachInferencePlan::default(),
            machines,
            states,
            calls,
        };
        let unknown = SymbolHandle::from_arena_index(99);

        assert_eq!(behavior.machine(unknown), Default::default());
        assert_eq!(behavior.state(unknown), Default::default());
        assert_eq!(behavior.call(unknown, 99, unknown), Default::default());
    }
}
