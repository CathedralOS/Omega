//! Reject any `EvaluateDispatchGuard` left in the selected-instruction plan
//! with `StateGuardLowering::UnresolvedInlineArmGuard`: the POISON selection
//! emits for an inline-leaf VALUE arm whose guard it could not resolve.
//! Before the poison existed such an arm was silently DROPPED -- no compare,
//! no result write -- so the value call returned a stale 0 (the slice-len
//! guard miscompile). This check turns the drop into a compile error.
//!
//! NOTE: `NeedsRuntimeExpression` guards are deliberately NOT rejected here;
//! dispatch edges use that lowering as an intentional zero-width
//! "unconditionally enter" fallthrough (e.g. the false arm of a
//! string-equality transition), which many green programs rely on.

use crate::EmissionPlanningInput;
use crate::semantic_scope::proof_scope_suffix;
use omega_backend_report_types::EmissionBlocker;
use omega_core::arena::Arena;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering};

use super::{blocker, semantic_scope::state_name};

pub(super) fn collect_unlowered_guard_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, instruction) in input.instructions.code.instructions.iter() {
        let SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::UnresolvedInlineArmGuard,
            ..
        } = instruction.kind
        else {
            continue;
        };

        blockers.insert(blocker(
            "state guards",
            &format!(
                "{} statement {} transition arm guard needs runtime guard lowering \
                 (the guard expression did not resolve to a comparable operand; \
                 emitting it as-is would silently drop the guarded arm){}",
                state_name(input, instruction.source_key),
                instruction.source_statement,
                proof_scope_suffix(input, instruction.source_key)
            ),
        ));
    }
}
