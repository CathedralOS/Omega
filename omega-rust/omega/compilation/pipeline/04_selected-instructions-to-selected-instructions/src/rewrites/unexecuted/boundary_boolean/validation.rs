//! Independent validation of boundary-decided predicate materialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the named instruction's
//! ordering `MaterializeBoolean*` kind and emitted `[def result]` shape,
//! the flag units it reads resolving through the shared condition-state
//! walk to one compare's published definitions, the compare's resolved
//! operand poles, and — for a decided predicate — the target's own
//! materialize row the rewritten instruction is built from — then
//! rebuilds the instruction the contract demands and requires the
//! proposal to carry it at the materialization's position. Reinserting
//! the source boolean must restore the complete source by content: every
//! other instruction, register, call, and function included. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::{
    BoundaryBooleanError, BoundaryBooleanReceipt, Outcome, ValidatedBoundaryBoolean, decidable_kind,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::ConditionStateError;
use crate::rewrites::sealed;

/// What the pole operand forces on the reader's predicate over
/// `left - right`, in the validator's own record: the same two outcomes
/// the contract defines, re-decided here so a wrong boundary decision
/// inside the producer's `outcome` cannot pass validation.

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted materialization's coordinates and emitted provenance, the
/// boolean result register and decided outcome, the target's materialize
/// row a constant fold is built from, the resolved-index successor lists
/// the shared walk ran over, and the flag-use count for the measured-step
/// contract. It shares no state with the producer's `admission` record.
use crate::rewrites::block_edges::member_position;
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    materialization_index: usize,
    materialization_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    result: VirtualRegisterId,
    outcome: Outcome,
    /// The materialize row the constant-folded instruction is built from —
    /// present only for `Outcome::Constant`; the equality collapse reuses
    /// the reader's own row and surface verbatim.
    row: Option<&'source RegisterInstructionConstraint>,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
}

/// The four ordering boolean kinds a pole can decide. `MaterializeBooleanEqual`
/// is absent: `left == right` varies with every unknown side, so no single
/// operand pole can fix it. The validator re-decides the emitted kinds
/// from the instruction kind alone — a wrong admission inside shared
/// producer code would decide both sides identically.

/// The boundary decision over the compare's resolved operand poles. The
/// strict orderings fold only at the far pole — a strict less-than against
/// the domain minimum, or from the domain maximum, can never hold — and the
/// remaining pole cases collapse to a negation (`x != bound`) that no
/// boolean materialization kind expresses. The inclusive orderings are
/// tautologies at the near pole and equalities at the far one. Constant
/// outcomes take precedence where two rules could fire: `(0, 0)` under
/// `U64LessOrEqual` is decided `true` rather than rewritten to the equality
/// reader that would compute the same value. This table is the fold's
/// legality decision: it lives here, not in `admission`, because a wrong
/// boundary rule in shared producer code would certify the proposal its
/// own bug emitted.

/// The boolean result register of an emitted `[def result]` flag reader,
/// or none: the validator re-decides the emitted shape — one of the four
/// ordering kinds a pole can decide, one plain definition, no implicit
/// definitions or clobbers, and a nonempty flag-use roster, since a
/// boolean observing no condition state has no predicate to decide. The
/// equality kind is refused with the shape: no pole can decide
/// `left == right`.
fn flag_reader(instruction: &SelectedInstruction) -> Option<VirtualRegisterId> {
    if !decidable_kind(instruction.kind)
        || instruction.operands.len() != 1
        || instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
    {
        return None;
    }
    let result = &instruction.operands[0];
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
    {
        return None;
    }
    Some(result.virtual_register)
}

/// The family's own mapping of a shared boundary-operand rejection onto
/// its error vocabulary. The producer's mapping lives inline in
/// `admission::admit`; the validator maps each site itself so a changed
/// producer mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> BoundaryBooleanError {
    match error {
        ConditionStateError::Use => BoundaryBooleanError::UnsupportedUse,
        ConditionStateError::Literal | ConditionStateError::Producer => {
            BoundaryBooleanError::UnsupportedLiteral
        }
    }
}

/// Reconstruct the legality of folding `materialization` from first
/// principles: locate the instruction, verify the emitted flag-reader
/// shape and the result register, resolve every used unit's reaching
/// event on the module's shared condition-state walk and require the one
/// compare site whose published definitions cover them all, prove the
/// compare's operand audit yields a pole that decides the reader's
/// predicate, and check the target's materialize row against the contract
/// a constant fold assumes. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, BoundaryBooleanError> {
    let (_, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            BoundaryBooleanError::SourceMismatch
        })?;
    let (block_index, materialization_index) =
        member_position(function, materialization).ok_or(BoundaryBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    let result = flag_reader(materialization_instruction)
        .ok_or(BoundaryBooleanError::UnsupportedInstruction)?;
    let result_register = function
        .virtual_register(result)
        .ok_or(BoundaryBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the materialization's block — only their entry sets can feed the
    // result.
    let derivation = super::derive_boolean_fold(
        function,
        block_index,
        materialization_index,
        materialization_instruction,
        result_register.class,
        environment,
        condition_error,
    )?;
    let outcome = derivation.outcome;
    let row = derivation.row;
    Ok(Reconstructed {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        outcome,
        row,
        successors: derivation.successors,
        implicit_uses: materialization_instruction.implicit_uses.len(),
    })
}

/// Build the instruction the contract demands from the validator's own
/// record: a decided predicate becomes the target's own materialize row
/// carrying the outcome, while the boolean's result register, instruction
/// identity, and provenance stay with the result; an equality collapse
/// keeps the source instruction whole — the flag-reading kinds share one
/// row — and swaps only its kind field for `MaterializeBooleanEqual`. The
/// producer's `rewritten` constructor is not consulted; both sides derive
/// the same instruction from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedInstruction {
    match reconstructed.outcome {
        Outcome::Constant(value) => {
            let row = reconstructed.row.expect("constant outcome carries its row");
            SelectedInstruction {
                id: reconstructed.materialization_id,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(u8::from(value))),
                },
                constraint: row.key,
                operands: row
                    .operands
                    .iter()
                    .zip([reconstructed.result])
                    .map(|(operand, register)| SelectedOperand {
                        operand: operand.operand,
                        virtual_register: register,
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
                provenance: reconstructed.provenance.clone(),
            }
        }
        Outcome::Equal => {
            let source = &reconstructed.function.blocks[reconstructed.block_index].instructions
                [reconstructed.materialization_index];
            let mut rewritten = source.clone();
            rewritten.kind = SelectedInstructionKind::MaterializeBooleanEqual;
            rewritten
        }
    }
}

/// Reinsert the source boolean: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, BoundaryBooleanError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(BoundaryBooleanError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(BoundaryBooleanError::ReplayMismatch)?;
    if block.instructions.len() <= reconstructed.materialization_index {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    block.instructions[reconstructed.materialization_index] = reconstructed.function.blocks
        [reconstructed.block_index]
        .instructions[reconstructed.materialization_index]
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// instruction at the materialization's position to equal the instruction
/// its own record produces, and restores the complete source by content —
/// every other instruction, register, call, and settlement included. The
/// producer's `admission::admit` is never consulted, so a wrong legality
/// decision fails here even when the proposal matches the edit the
/// producer emitted.
pub(crate) fn validate_boundary_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBoundaryBoolean, BoundaryBooleanError> {
    let reconstructed = reconstruct(source, function_index, materialization, environment)?;
    if u64::try_from(super::measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.implicit_uses,
    )?)
    .map_err(|_| BoundaryBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(BoundaryBooleanError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .and_then(|block| block.instructions.get(reconstructed.materialization_index))
        != Some(&expected(&reconstructed))
    {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedBoundaryBoolean,
        BoundaryBooleanReceipt,
        source,
        proposed
    ))
}
