//! Shared source-to-rewrite fixtures for cross-stage allocation controls.
//!
//! The rewrite corpus keeps one fixture language across rule families: the
//! dominant work budget, measured step-boundary budgets, and the
//! constraint-row instruction builder every hand-built plan shares, so a rule
//! file holds only the identifiers and shapes that distinguish its subject.

use optimization_core::OptimizationWorkBudget;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedOperand,
    VirtualRegisterId,
};

pub use crate::rewrites::allocation_recovery::pressure_rematerialization::tests::{
    multiple_use::exercise_multiple_use_rematerialization,
    sole_use::exercise_single_use_rematerialization,
};

/// The generous fixture budget shared by the rewrite corpus: wide enough that
/// no exercised plan reaches it, so budget assertions stay about the measured
/// axes rather than incidental totals.
pub fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

/// A budget carrying `steps` measured steps on a one-everything-else profile,
/// for the measured validation-step boundary legs: the exact count admits,
/// `measured_step_budget(steps - 1)` starves.
pub fn measured_step_budget(steps: u64) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap()
}

/// Build a selected instruction from its constraint row and bound registers.
pub fn instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    registers: &[VirtualRegisterId],
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: *register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

/// Retain a checked run produced with an explicit test register budget.
pub fn retain_selected_lowering_run(
    run: crate::StagedSelectedLoweringOptimizationRun,
) -> Result<crate::SelectedInstructionOptimizationOutput, crate::SelectedInstructionOptimizationError>
{
    crate::SelectedInstructionOptimizationOutput::from_evidence(
        crate::SelectedInstructionOptimizationEvidence::LiteralFolds(run),
    )
}
