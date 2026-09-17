//! Shared admission for dead-compare removal: locate the named compare,
//! confirm its canonical flag-publisher shape against the target's own
//! constraint row, and prove every condition-state unit it publishes dies
//! unread on every forward path.
//!
//! The compare kinds are the only instructions whose complete observable
//! effect is the implicit flag surface they publish — every other kind
//! carries a scalar result, a memory effect, or a control transfer that a
//! removal would silently drop. Once each published unit is proven dead —
//! killed by a redefinition or clobber, or carried out of the function,
//! before any implicit use can observe it — the instruction computes
//! nothing and leaves the plan.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};

use super::DeadCompareError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub compare_index: usize,
}

/// Every instruction of `block`, including the one its terminator carries:
/// a flag read there still observes the compare's definitions, and a flag
/// event there still ends a unit's live range.
fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// Every successor edge of `block`'s terminator. A flag unit still live at
/// the terminator's end can be observed by a reader at the head of any of
/// them; the edge's own transports move registers and storage slots, never
/// condition-state units.
fn successors(block: &SelectedBlock) -> impl Iterator<Item = &SelectedSuccessor> {
    match &block.terminator {
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None].into_iter().flatten(),
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        } => [Some(when_nonzero), Some(when_zero)].into_iter().flatten(),
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            [None, None].into_iter().flatten()
        }
    }
}

/// Whether `kind` is one of the three flag publishers whose whole effect is
/// the condition-state surface it defines, with the operand arity the
/// emitted form carries.
fn compare_operand_arity(kind: SelectedInstructionKind) -> Option<usize> {
    match kind {
        SelectedInstructionKind::CompareI64 => Some(2),
        SelectedInstructionKind::CompareI64Immediate { .. }
        | SelectedInstructionKind::CompareI64Zero => Some(1),
        _ => None,
    }
}

/// Audit one condition-state unit the compare publishes: walk forward from
/// the instruction after the compare until an implicit definition or
/// clobber ends the unit's live range on every path. Any reached implicit
/// use refuses — the compare's publication is observable there. A unit
/// still live past the terminator resumes at the head of each successor
/// block; an edge naming a block the function does not contain leaves the
/// unit's readers unprovable and refuses; a terminator with no successors
/// ends the path with the unit unobserved. `(block, start)` pairs dedupe
/// the walk, so a loop carrying the unit back into the compare's own block
/// re-scans from its head — a reader there observes the compare's earlier
/// execution, and the compare's own definition ends the unit past it.
fn audit_dead_unit(
    function: &SelectedFunction,
    block_index: usize,
    compare_index: usize,
    unit: RegisterUnitId,
) -> Result<(), DeadCompareError> {
    let mut visited = BTreeSet::new();
    let mut frontier = vec![(block_index, compare_index + 1)];
    while let Some((current, start)) = frontier.pop() {
        if !visited.insert((current, start)) {
            continue;
        }
        let block = &function.blocks[current];
        let mut killed = false;
        for instruction in block_instructions(block).skip(start) {
            if instruction.implicit_uses.contains(&unit) {
                return Err(DeadCompareError::UnsupportedUse);
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                killed = true;
                break;
            }
        }
        if killed {
            continue;
        }
        for successor in successors(block) {
            let target = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
                .ok_or(DeadCompareError::UnsupportedUse)?;
            frontier.push((target, 0));
        }
    }
    Ok(())
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, DeadCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DeadCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DeadCompareError::SourceMismatch)?;
    let (block_index, compare_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == compare)
                .map(|compare_index| (block_index, compare_index))
        })
        .ok_or(DeadCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in the
    // emitted flag-publisher shape: every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently drop,
    // and a nonempty published surface — a compare defining no condition
    // state at all is malformed, not dead.
    let arity = compare_operand_arity(compare_instruction.kind)
        .ok_or(DeadCompareError::UnsupportedInstruction)?;
    if compare_instruction.operands.len() != arity
        || !compare_instruction.implicit_uses.is_empty()
        || compare_instruction
            .implicit_defs
            .iter()
            .chain(compare_instruction.clobbers.iter())
            .next()
            .is_none()
        || compare_instruction
            .operands
            .iter()
            .enumerate()
            .any(|(position, operand)| {
                operand.operand != position as u16
                    || operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_some()
                    || operand.tied_to.is_some()
                    || operand.early_clobber
            })
    {
        return Err(DeadCompareError::UnsupportedInstruction);
    }
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees would
    // change what removing the instruction means.
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(DeadCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(DeadCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(DeadCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(DeadCompareError::ConstraintMismatch);
        }
    }
    // Removing the compare orphans anything that names it: call contracts
    // and memory-access rows bind instructions by identity, and neither may
    // claim the compare.
    if function
        .calls
        .iter()
        .any(|call| call.instruction == compare)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == compare)
    {
        return Err(DeadCompareError::UnsupportedInstruction);
    }
    // Every condition-state unit the compare publishes must die unread:
    // each walks forward from the compare until a redefinition or clobber
    // kills it on every path, refusing the first reached reader.
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    for &unit in &units {
        audit_dead_unit(function, block_index, compare_index, unit)?;
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(DeadCompareError::IdentityOverflow)?;
    // The dead-unit audit visits each block once per entry position per
    // published unit. Only the compare's own block carries a nonzero start,
    // so two visits per block per unit bound the scan; each visit walks the
    // body's instructions and terminator plus, when the unit stays live,
    // the successor edges.
    let edge_count = function
        .blocks
        .iter()
        .map(|block| successors(block).count())
        .sum::<usize>();
    let dead_audit = units
        .len()
        .checked_mul(
            function_scan
                .checked_mul(2)
                .and_then(|scan| scan.checked_add(edge_count))
                .ok_or(DeadCompareError::IdentityOverflow)?,
        )
        .ok_or(DeadCompareError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            // A second pass of this function audits the operand classes,
            // roster rows, and call/access rows the compare must not be
            // named by.
            total
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| total.checked_add(dead_audit))
        .ok_or(DeadCompareError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DeadCompareError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DeadCompareError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        compare_index,
    })
}

/// The one-function transformation shared by proposal and replay: remove
/// the compare and shift boundary settlements over the removed ordinal.
/// Both sides compute it from the source, never from each other.
pub(super) fn apply(
    admitted: &Admission<'_>,
    function: &mut SelectedFunction,
) -> Result<(), DeadCompareError> {
    let block = function
        .blocks
        .get_mut(admitted.block_index)
        .ok_or(DeadCompareError::SourceMismatch)?;
    if block.id != admitted.block {
        return Err(DeadCompareError::SourceMismatch);
    }
    function.boundary_settlements =
        shifted_boundary_settlements(admitted.function, admitted.block, admitted.compare_index)?;
    block.instructions.remove(admitted.compare_index);
    Ok(())
}

/// The block's boundary settlements after removing the instruction at
/// `removed`: positions at or before it name instructions that stay put,
/// and every later position — including the after-body position — shifts
/// one ordinal earlier. Settlements name hosted boundary operations, never
/// flag units or registers, so the removal itself is invisible to them.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    block: SelectedBlockId,
    removed: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, DeadCompareError> {
    let body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(DeadCompareError::SourceMismatch)?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(DeadCompareError::SourceMismatch);
        }
        if position > removed {
            settlement.instruction_index =
                u32::try_from(position - 1).map_err(|_| DeadCompareError::IdentityOverflow)?;
        }
    }
    Ok(shifted)
}
