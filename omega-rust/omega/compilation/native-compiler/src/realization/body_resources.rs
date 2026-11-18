// Compose the closed physical call graph before any body is encoded. Only
// callbacks actually invoked by a retained foreign call contribute their entry
// cost, matching the independent object readback route.

use super::callback_thunks::PlannedPrivateBody;
use crate::pipeline::PreparedPhysicalBody;
use diagnostics::Diagnostic;
use physical_planning::stack_demand::{StackDemand, derive_physical_stack_demand};
use provider_admission::provider_admission::realization_error;

pub(super) fn plan(
    body: &PreparedPhysicalBody,
    private: &[PlannedPrivateBody],
) -> Result<StackDemand, Vec<Diagnostic>> {
    visit(body, private, &mut Vec::new())
}

fn visit(
    body: &PreparedPhysicalBody,
    private: &[PlannedPrivateBody],
    active: &mut Vec<function_identity::MachineFunctionIdentity>,
) -> Result<StackDemand, Vec<Diagnostic>> {
    let invalid = |reason| realization_error("physical program stack", reason);
    let mut demands = Vec::new();
    for identity in body
        .facts
        .stack
        .functions
        .iter()
        .flat_map(|function| &function.foreign_calls)
        .flat_map(|call| &call.invoked_callbacks)
    {
        if demands.iter().any(|(known, _)| known == identity) {
            continue;
        }
        let private_body = private
            .iter()
            .find(|body| body.identity == *identity)
            .ok_or_else(|| invalid("callback stack requirement has no retained physical body"))?;
        if active.contains(identity) {
            return Err(invalid(
                "callback stack demand has a cyclic executable dependency",
            ));
        }
        active.push(*identity);
        let mut demand = visit(&private_body.callable.body, private, active)?;
        if let Some(bridge) = private_body.entry.bridge() {
            let alignment = demand.stack_alignment();
            demand = demand
                .with_entry_cost(
                    u64::from(bridge.frame_bytes()) + u64::from(bridge.call_stack_bias()),
                    alignment,
                )
                .ok_or_else(|| invalid("private entry stack demand overflows"))?;
        }
        active.pop();
        demands.push((*identity, demand));
    }
    derive_physical_stack_demand(&body.facts.stack, body.frame.instructions.entry, demands)
        .map_err(|error| realization_error("physical program stack", format!("{error:?}")))
}
