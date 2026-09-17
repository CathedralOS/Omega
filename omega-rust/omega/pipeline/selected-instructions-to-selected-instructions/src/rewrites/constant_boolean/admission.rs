//! Shared admission for constant condition materialization: locate the
//! named `MaterializeBoolean*`, confirm its clean `[def result]` shape,
//! resolve every implicit flag unit it reads to the same compare
//! definition — in-block or across predecessor edges — and prove that
//! compare's operands compile-time constant so the observed predicate is
//! decidable.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::ConstantBooleanError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, constant_operands, entry_index, instruction_at,
    reaching_event,
};

impl From<ConditionStateError> for ConstantBooleanError {
    fn from(error: ConditionStateError) -> Self {
        match error {
            ConditionStateError::Use => ConstantBooleanError::UnsupportedUse,
            ConditionStateError::Producer => ConstantBooleanError::UnsupportedProducer,
            ConditionStateError::Literal => ConstantBooleanError::UnsupportedLiteral,
        }
    }
}

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The boolean result register: the rewritten materialization defines
    /// it unchanged.
    pub result: VirtualRegisterId,
    /// The folded predicate outcome the rewritten `MaterializeI64` carries.
    pub value: bool,
    /// The materialize row the rewritten instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
}

/// The constant a flag-reading `MaterializeBoolean*` materializes when the
/// compare observed `(left, right)` — the predicates the selected catalog
/// defines over `left - right`, evaluated at compile time.
fn predicate_outcome(kind: SelectedInstructionKind, left: u64, right: u64) -> Option<bool> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanEqual => Some(left == right),
        SelectedInstructionKind::MaterializeBooleanU64LessThan => Some(left < right),
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            Some((left as i64) < (right as i64))
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => Some(left <= right),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Some((left as i64) <= (right as i64))
        }
        _ => None,
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBooleanError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let (block_index, materialization_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == materialization)
                .map(|materialization_index| (block_index, materialization_index))
        })
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be the emitted `[def result]` flag reader:
    // one plain definition, no implicit definitions or clobbers, and a
    // nonempty flag-use roster — a boolean observing no condition state has
    // no constant to evaluate.
    if predicate_outcome(materialization_instruction.kind, 0, 0).is_none()
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result_operand = &materialization_instruction.operands[0];
    if result_operand.operand != 0
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.fixed_view.is_some()
        || result_operand.tied_to.is_some()
        || result_operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|entry| entry.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // materialization's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave the
    // observed value unknown. Other readers of the same units are
    // unaffected: the compare stays.
    let mut compare_site = None;
    for unit in &materialization_instruction.implicit_uses {
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(ConstantBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let (left, right) = constant_operands(function, compare_instruction)?;
    let value = predicate_outcome(materialization_instruction.kind, left, right)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    // The rewritten instruction is the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had.
    let row = environment
        .constraint(environment.selected_keys().materialize_i64)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Def
        || row.operands[0].class != result_register.class
        || row.operands[0].fixed_view.is_some()
        || row.operands[0].tied_to.is_some()
        || row.operands[0].early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each used unit then scans the materialization block's prefix
    // and — on an in-block miss — every block's stream for its last event,
    // after which entry-set propagation requeues a block only while its
    // set grows: a set holds at most one element per event site plus the
    // unknown marker, so pops stay under `blocks × (elements + 1)` and
    // each pop visits its out-edges — no more than the widest terminator's
    // — at a bounded union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let reach_scan = materialization_instruction
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        value,
        row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// materialize row supplies the operand interface while the boolean's
/// result register, instruction identity, and provenance stay with the
/// result. The folded constant is the predicate outcome the retained
/// compare publishes.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.materialization_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(u8::from(admitted.value))),
        },
        constraint: admitted.row.key,
        operands: admitted
            .row
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
        implicit_uses: admitted.row.implicit_uses.clone(),
        implicit_defs: admitted.row.implicit_defs.clone(),
        clobbers: admitted.row.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
