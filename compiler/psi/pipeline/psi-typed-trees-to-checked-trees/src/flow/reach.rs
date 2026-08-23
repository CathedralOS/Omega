use psi_checked_trees::{FlowControlFacts, FlowFacts};
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};

pub(super) fn attach_reach_summaries(
    flow: &mut FlowFacts,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    operational: &psi_effects::OperationalPlan,
) {
    let FlowControlFacts { states, calls, .. } = &mut flow.control;
    states.for_each_mut(|_, state| {
        let service_state = service_reaches.for_state(state.state_symbol);
        state.service_reach = service_state
            .map(|summary| ServiceReachSummary {
                direct: summary.inferred_direct,
                transitive: summary.inferred_transitive,
            })
            .unwrap_or_default();

        let operational_state = operational
            .machines()
            .iter()
            .flat_map(|machine| operational.states.span_or_empty(machine.states))
            .find(|summary| summary.symbol == state.state_symbol);
        state.suspension = operational_state
            .map(|summary| {
                let direct_may_suspend = summary.direct_may_suspend
                    || operational
                        .calls
                        .span_or_empty(summary.calls)
                        .iter()
                        .any(|call| call.direct_may_suspend);
                SuspensionSummary {
                    direct_may_suspend,
                    transitive_may_suspend: summary.transitive_may_suspend,
                }
            })
            .unwrap_or_default();
        state.blocking = operational_state
            .map(|summary| {
                let direct_may_block = summary.direct_may_block
                    || operational
                        .calls
                        .span_or_empty(summary.calls)
                        .iter()
                        .any(|call| call.direct_may_block);
                BlockingSummary {
                    direct_may_block,
                    transitive_may_block: summary.transitive_may_block,
                }
            })
            .unwrap_or_default();

        for call in calls.span_mut_or_empty(state.calls) {
            let service_call = service_state.and_then(|state| {
                service_reaches.calls_for(state).iter().find(|summary| {
                    summary.statement_index == call.statement_index
                        && summary.call_ordinal == call.call_ordinal
                        && summary.target_state == call.target_symbol
                })
            });
            call.service_reach = service_call
                .map(|summary| ServiceReachSummary {
                    direct: summary.inferred_direct,
                    transitive: summary.inferred_transitive,
                })
                .unwrap_or_default();

            let operational_call = operational_state.and_then(|state| {
                operational
                    .calls
                    .span_or_empty(state.calls)
                    .iter()
                    .find(|summary| {
                        summary.statement_index == call.statement_index
                            && summary.call_ordinal == call.call_ordinal
                            && summary.target_state_symbol == call.target_symbol
                    })
            });
            call.suspension = operational_call
                .map(|summary| SuspensionSummary {
                    direct_may_suspend: summary.direct_may_suspend,
                    transitive_may_suspend: summary.transitive_may_suspend,
                })
                .unwrap_or_default();
            call.blocking = operational_call
                .map(|summary| BlockingSummary {
                    direct_may_block: summary.direct_may_block,
                    transitive_may_block: summary.transitive_may_block,
                })
                .unwrap_or_default();
            call.operational_acknowledgement = operational_call
                .map(|summary| summary.acknowledgement)
                .unwrap_or_default();
        }
    });
}
