// Compose the closed physical call graph before any body is encoded. Only
// callbacks actually invoked by a retained foreign call contribute their entry
// cost, matching the independent object readback route.

use super::callback_thunks::PlannedPrivateBody;
use crate::driver::compile_body::PreparedPhysicalBody;
use diagnostics::Diagnostic;
use frame_planning::stack_demand::StackDemand;
use image_emission::callback_stack::{self, CallbackStackBody};
use provider_admission::realization_error;

pub(super) fn plan(
    body: &PreparedPhysicalBody,
    private: &[PlannedPrivateBody],
) -> Result<StackDemand, Vec<Diagnostic>> {
    callback_stack::derive(
        describe(body, 0),
        private,
        |body| body.identity,
        |private| {
            let cost = private.entry.bridge().map_or(0, |bridge| {
                u64::from(bridge.frame_bytes()) + u64::from(bridge.call_stack_bias())
            });
            describe(&private.callable.body, cost)
        },
    )
    .map_err(|error| {
        let reason = match error {
            callback_stack::Error::MissingBody => {
                "callback stack requirement has no retained physical body"
            }
            callback_stack::Error::Cycle(_) => {
                "callback stack demand has a cyclic executable dependency"
            }
            callback_stack::Error::EntryCostOverflow(_) => "private entry stack demand overflows",
            callback_stack::Error::Body(error) => {
                return realization_error("physical program stack", format!("{error:?}"));
            }
        };
        realization_error("physical program stack", reason)
    })
}

fn describe(body: &PreparedPhysicalBody, entry_cost: u64) -> CallbackStackBody<'_> {
    let program = &body.stage.facts().physical.stack;
    CallbackStackBody {
        program,
        entry: body.stage.program().frame.instructions.entry,
        callbacks: program
            .functions
            .iter()
            .flat_map(|function| &function.foreign_calls)
            .flat_map(|call| call.invoked_callbacks.iter().copied())
            .collect(),
        entry_cost,
    }
}
