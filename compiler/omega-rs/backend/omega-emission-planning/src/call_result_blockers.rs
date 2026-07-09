use crate::EmissionPlanningInput;
use crate::blocker;
use crate::semantic_scope::state_name;
use omega_backend_report_types::EmissionBlocker;
use omega_core::arena::Arena;
use omega_target_operations::SelectedInstructionKind;

/// Every dispatch-loop edge that carries a `CallResultReturn` must have a
/// SELECTED return-write (integer/copy/binary) at its clone-terminal state --
/// otherwise the caller's result slot silently keeps its prior/ZII value (the
/// exact silent-wrong class the return-write matrix has been closing: field
/// bindings, binary terminals, transition args). A terminal shape the
/// return-write cannot serve yet (float terminals, unresolvable places) now
/// refuses LOUDLY here instead of misdelivering -- which is what makes the
/// splice fences' dispatch-route exemption sound: dispatched value calls
/// either deliver correctly or fail to compile, never silently ZII.
pub(crate) fn collect_call_result_return_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, case) in input.runtime_dispatch_loop.cases.iter() {
        for edge in input
            .runtime_dispatch_loop
            .edges
            .span(case.edges)
            .into_iter()
            .flatten()
        {
            let Some(call_result) = edge.call_result else {
                continue;
            };
            let served = input.instructions.code.instructions.iter().any(
                |(_, instruction)| {
                    instruction.source_key.machine == case.key.machine
                        && instruction.source_key.state == case.key.state
                        && matches!(
                            instruction.kind,
                            SelectedInstructionKind::WriteRuntimeStorageInteger { .. }
                                | SelectedInstructionKind::CopyRuntimeStorage { .. }
                                | SelectedInstructionKind::WriteRuntimeStorageBinary { .. }
                                // The slice-element terminal serve (`-> s[j]`,
                                // 2026-07-09k2): region-paired indexed copies.
                                | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
                                | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage { .. }
                        )
                },
            );
            if served {
                continue;
            }
            blockers.insert(blocker(
                "call result",
                &format!(
                    "{}: the dispatched value call's terminal (returning into {} \
                     statement {}) has no selected return-write -- this terminal \
                     shape is not served yet (float terminal, or an unresolvable \
                     value), and running it would silently leave the caller's \
                     result as ZII. Bind through a supported shape (integer \
                     place/literal/binary terminal) or restructure the callee.",
                    state_name(input, case.key),
                    state_name(input, call_result.call_source_key),
                    call_result.statement_index,
                ),
            ));
        }
    }
}
