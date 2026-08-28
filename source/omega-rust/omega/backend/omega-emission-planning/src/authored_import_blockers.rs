use crate::EmissionPlanningInput;
use crate::blocker;
use crate::semantic_scope::state_name;
use omega_backend_report_types::EmissionBlocker;
use omega_calling_conventions::{HostBindingMechanism, HostCapability};
use psi_arena::Arena;

/// A source-authored external DllImport call (operation key outside the closed
/// catalog -- `(Unknown, Unknown)`) rides the GENERAL value-returning import
/// encoder, whose operand[0] is the RESULT place. A statement-position call
/// (`self.beeper.beep(v);`) has no prepended result, so its first ARGUMENT
/// would be misread as the result place: the call runs with shifted arguments
/// and clobbers a local -- silently. Until a void authored-import lowering
/// exists, require the binding-the-result shape. Authored VtableSlot calls
/// are unaffected (their encoder is void-shaped -- the EFI console path).
pub(crate) fn collect_authored_import_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in input.host_calls.calls.iter() {
        if host_call.has_result {
            continue;
        }
        let authored_import = input
            .host_calls
            .operations
            .span(host_call.operations)
            .unwrap_or(&[])
            .iter()
            .any(|operation| {
                matches!(
                    operation.operation_key.capability,
                    HostCapability::Unknown | HostCapability::Custom(_)
                ) && input.host_abi.bindings.iter().any(|(_, binding)| {
                    binding.operation_key == operation.operation_key
                        && matches!(binding.mechanism, HostBindingMechanism::Import { .. })
                })
            });
        if !authored_import {
            continue;
        }
        blockers.insert(blocker(
            "host lowering",
            &format!(
                "{} statement {}: a source external import is called as a \
                 STATEMENT -- the general import lowering stores a result, so \
                 the call would misread its first argument as the result place. \
                 Bind the result to a value (`let rc: i32 = ...;`) even if you \
                 ignore it.",
                state_name(input, host_call.source_key),
                host_call.statement_index,
            ),
        ));
    }
}
