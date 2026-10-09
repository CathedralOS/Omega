//! Shared admission for boundary-decided predicate materialization: locate
//! the named ordering `MaterializeBoolean*`, confirm its clean
//! `[def result]` shape, resolve every implicit flag unit it reads to the
//! same compare definition — in-block or across predecessor edges — and
//! prove one compare operand sits at a carrier-domain pole that fixes the
//! predicate, or collapses it to equality, without consulting the other.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::{BoundaryBooleanError, Outcome, decidable_kind};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::member_position;

/// What the pole operand forces on the reader's predicate over
/// `left - right`.

/// The four ordering boolean kinds a pole can decide. `MaterializeBooleanEqual`
/// is absent: `left == right` varies with every unknown side, so no single
/// operand pole can fix it.

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The boolean result register: the rewritten materialization defines
    /// it unchanged under either outcome.
    pub result: VirtualRegisterId,
    pub outcome: Outcome,
    /// The materialize row the constant-folded instruction is built from —
    /// present only for `Outcome::Constant`; the equality collapse reuses
    /// the reader's own row and surface verbatim.
    pub row: Option<&'source RegisterInstructionConstraint>,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, BoundaryBooleanError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            BoundaryBooleanError::SourceMismatch
        })?;
    let (block_index, materialization_index) =
        member_position(function, materialization).ok_or(BoundaryBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be the emitted `[def result]` flag reader:
    // one plain definition, no implicit definitions or clobbers, and a
    // nonempty flag-use roster — a boolean observing no condition state has
    // no predicate to decide. The equality kind is refused with the shape:
    // no pole can decide `left == right`.
    if !decidable_kind(materialization_instruction.kind)
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(BoundaryBooleanError::UnsupportedInstruction);
    }
    let result_operand = &materialization_instruction.operands[0];
    if result_operand.operand != 0
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.fixed_view.is_some()
        || result_operand.tied_to.is_some()
        || result_operand.early_clobber
    {
        return Err(BoundaryBooleanError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    let result_register = function
        .virtual_register(result)
        .ok_or(BoundaryBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // materialization's block — only their entry sets can feed the result.
    let derivation = super::derive_boolean_fold(
        function,
        block_index,
        materialization_index,
        materialization_instruction,
        result_register.class,
        environment,
        |error| match error {
            crate::rewrites::condition_state::ConditionStateError::Use => {
                BoundaryBooleanError::UnsupportedUse
            }
            crate::rewrites::condition_state::ConditionStateError::Literal
            | crate::rewrites::condition_state::ConditionStateError::Producer => {
                BoundaryBooleanError::UnsupportedLiteral
            }
        },
    )?;
    let outcome = derivation.outcome;
    let row = derivation.row;
    let steps = super::measured_steps(
        plan,
        function,
        &derivation.successors,
        materialization_instruction.implicit_uses.len(),
    )?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || BoundaryBooleanError::IdentityOverflow,
        || BoundaryBooleanError::WorkBudgetExceeded,
    )?;
    Ok(Admission {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        outcome,
        row,
    })
}

/// The one-instruction proposal shape shared with replay. A decided
/// predicate becomes the target's own materialize row carrying the outcome,
/// while the boolean's result register, instruction identity, and
/// provenance stay with the result. An equality collapse keeps the source
/// instruction whole — the flag-reading kinds share one row — and swaps
/// only its kind field for `MaterializeBooleanEqual`.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    match admitted.outcome {
        Outcome::Constant(value) => {
            let row = admitted.row.expect("constant outcome carries its row");
            SelectedInstruction {
                id: admitted.materialization_id,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(u8::from(value))),
                },
                constraint: row.key,
                operands: row
                    .operands
                    .iter()
                    .zip([admitted.result])
                    .map(
                        |(operand, register)| selected_instructions::SelectedOperand {
                            operand: operand.operand,
                            virtual_register: register,
                            access: operand.access,
                            class: operand.class,
                            fixed_view: operand.fixed_view,
                            tied_to: operand.tied_to,
                            early_clobber: operand.early_clobber,
                        },
                    )
                    .collect(),
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: admitted.provenance.clone(),
            }
        }
        Outcome::Equal => {
            let source = &admitted.function.blocks[admitted.block_index].instructions
                [admitted.materialization_index];
            let mut rewritten = source.clone();
            rewritten.kind = SelectedInstructionKind::MaterializeBooleanEqual;
            rewritten
        }
    }
}
