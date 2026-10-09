//! Shared admission for boundary-decided conditional-branch folding:
//! locate the named strict-ordering conditional-branch terminator, confirm
//! its emitted zero-operand shape, partition its implicit uses by the flag
//! universe — flag units resolve to the one compare through the shared
//! reaching walk, non-flag units must lie in the jump row's implicit
//! surface when the fold lands on `Jump` — and decide the outcome from the
//! compare's operand poles: a far pole or two known operands select the
//! `Jump`'s successor outright, while a near pole collapses the ordering
//! to the nonzero condition on the identical published flag state.
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::ConditionStateError;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedTerminator, terminator_instruction,
};
