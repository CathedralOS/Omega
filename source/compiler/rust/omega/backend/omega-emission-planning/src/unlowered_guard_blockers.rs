//! Reject the POISON `EvaluateDispatchGuard` markers left in the
//! selected-instruction plan:
//! - `UnresolvedInlineArmGuard`: an inline-leaf VALUE arm whose guard could
//!   not resolve. Before the poison such an arm was silently DROPPED -- no
//!   compare, no result write -- so the value call returned a stale 0 (the
//!   slice-len guard miscompile).
//! - `UnloweredTerminalHostCall`: a host-boundary call as a machine/state
//!   TERMINAL value. No write strategy lowers it, so the call silently never
//!   ran and its result slot read ZII 0 (`Filesystem::close` reported rc 0
//!   "success" while the fd stayed OPEN -- Windows' unlink-refuses-open-files
//!   exposed what POSIX unlink masked).
//! - `UnloweredCaseLiteralField`: a case/struct-literal payload field whose
//!   value no operand strategy lowers. The construction cascade OR'd
//!   per-field success, so the one bad field was dropped while the tag and
//!   siblings landed and the field read ZII 0 (first the cast-in-payload
//!   face, then text-equality payloads).
//! Each check turns a silent drop into a compile error.
//!
//! NOTE: `NeedsRuntimeExpression` guards are deliberately NOT rejected here;
//! dispatch edges use that lowering as an intentional zero-width
//! "unconditionally enter" fallthrough (e.g. the false arm of a
//! string-equality transition), which many green programs rely on.

use crate::EmissionPlanningInput;
use crate::semantic_scope::proof_scope_suffix;
use omega_backend_report_types::EmissionBlocker;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering};
use psi_arena::Arena;

use super::{blocker, semantic_scope::state_name};

pub(super) fn collect_unlowered_guard_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, instruction) in input.instructions.code.instructions.iter() {
        let SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering,
            operator,
            ..
        } = instruction.kind
        else {
            continue;
        };
        // A NeedsRuntimeExpression guard carrying a REAL comparison operator is
        // an unlowered comparison the emitter drops entirely -- the arm is
        // entered unconditionally (the value-call guard subject's always-true
        // face: `transition self.dbl(5) == 11` took the true arm). Operator
        // None is the legitimate zero-width "unconditionally enter"
        // fallthrough (a `_` arm / a string-equality false arm) and stays
        // accepted.
        if matches!(guard_lowering, StateGuardLowering::NeedsRuntimeExpression)
            && !matches!(operator, omega_target_operations::StateGuardOperator::None)
        {
            blockers.insert(blocker(
                "state guards",
                &format!(
                    "{} statement {} transition guard comparison has no runtime \
                     lowering (a value-machine call in the guard subject is not \
                     materialized -- the comparison would be silently dropped and \
                     the arm entered unconditionally). Bind the call result to a \
                     `let` local first, then guard the local{}",
                    state_name(input, instruction.source_key),
                    instruction.source_statement,
                    proof_scope_suffix(input, instruction.source_key)
                ),
            ));
            continue;
        }
        match guard_lowering {
            StateGuardLowering::UnresolvedInlineArmGuard => {
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
            StateGuardLowering::UnloweredTerminalHostCall => {
                blockers.insert(blocker(
                    "state values",
                    &format!(
                        "{} statement {}: a call is the TERMINAL value, which has no \
                         native value-return lowering here -- host-boundary calls are \
                         never served in terminal position, and a value-machine call \
                         under a GUARDED arm is not auto-hoisted (the hoist would run \
                         the callee even when the arm is not taken; ALWAYS arms hoist \
                         automatically). The call would silently never run and its \
                         result would read 0 (ZII). Bind it to a `let` and return the \
                         local (`let rc: i32 = self.host.op(..); rc`){}",
                        state_name(input, instruction.source_key),
                        instruction.source_statement,
                        proof_scope_suffix(input, instruction.source_key)
                    ),
                ));
            }
            StateGuardLowering::UnloweredCaseLiteralField => {
                blockers.insert(blocker(
                    "state values",
                    &format!(
                        "{} statement {}: a case-literal payload field's value has no \
                         operand lowering -- the field write would be silently dropped \
                         while the tag and sibling fields land, and the field would read \
                         0 (ZII). Bind the field value to a `let` local first, then name \
                         the local in the literal \
                         (`let v: bool = self.name == \"x\"; .. Case {{ flag: v }}`){}",
                        state_name(input, instruction.source_key),
                        instruction.source_statement,
                        proof_scope_suffix(input, instruction.source_key)
                    ),
                ));
            }
            _ => {}
        }
    }
}
