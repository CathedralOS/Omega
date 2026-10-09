//! Independent validation of boundary-decided conditional-branch folding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the terminator-carried
//! instruction's strict-ordering branch/kind pairing and emitted
//! zero-operand shape, the flag partition every implicit use must satisfy
//! (flag units resolving through the shared condition-state walk to the
//! one compare whose published definitions cover them, and — for the
//! decided `Jump` outcome — every other observed unit carried on the jump
//! row's own implicit surface), the compare's resolved operand poles, and
//! the pole-outcome table deciding between a `Jump` fold and the
//! `ConditionalBranchNonZero` collapse — then rebuilds the terminator the
//! contract demands and requires the proposal to carry it in the branch's
//! block. Reinserting the source terminator must restore the complete
//! source by content: every other instruction, register, call,
//! settlement, and function included. A producer admission error
//! therefore fails validation even when the proposal is exactly what that
//! producer emitted.
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedTerminator,
    terminator_instruction,
};
