//! Attach service-reach, suspension, and blocking summaries from the service-reach
//! inference and operational plans to each finished state and call fact, and each
//! call's operational acknowledgement.
use checked_trees::{FlowControlFacts, FlowFacts};
use language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};

pub(crate) fn attach_reach_summaries(
    program: &typed_trees::TypedTrees,
    flow: &mut FlowFacts,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    operational: &flow_effects::OperationalPlan,
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
            // Flow keeps the authored callable identity; effect inference
            // summarizes its executable entry. Resolve that association
            // without rewriting the occurrence or dropping its effects.
            let target_state = program
                .machine_by_symbol(call.target_symbol)
                .and_then(|machine| program.machine_states(machine).first())
                .map_or(call.target_symbol, |entry| entry.symbol);
            let service_call = service_state.and_then(|state| {
                service_reaches.calls_for(state).iter().find(|summary| {
                    summary.statement_index == call.statement_index
                        && summary.call_ordinal == call.call_ordinal
                        && summary.target_state == target_state
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
                            && summary.target_state_symbol == target_state
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

#[cfg(test)]
mod tests {
    #[test]
    fn named_machine_call_retains_its_published_service_ceiling() {
        let checked = crate::tests::front_end::checked_program(
            r#"
            boundary trait Device { machine read() -> u64; }
            machine endpoint() -> u64 reaches Device { 7 }
            machine choose() -> u64 reaches Device {
                transition { _ -> endpoint() }
            }
            "#,
        );
        let endpoint = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "endpoint")
            .unwrap();
        let call = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .map(|(_, call)| call)
            .find(|call| call.target_symbol == endpoint.symbol)
            .expect("flow retains the authored machine head");
        let published = checked
            .facts
            .service_reaches
            .for_machine(endpoint.symbol)
            .expect("callee service inference")
            .published_ceiling;
        assert!(
            !checked
                .facts
                .service_reaches
                .rows
                .services(published)
                .is_empty()
        );
        assert_eq!(call.service_reach.transitive, published);
    }
}
