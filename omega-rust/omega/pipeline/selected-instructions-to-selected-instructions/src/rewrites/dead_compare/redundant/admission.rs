//! Shared admission for redundant compare removal: locate the named
//! compare, confirm its canonical flag-publisher shape against the target's
//! own constraint row, resolve every published unit's last in-block flag
//! event to one flag-equivalent earlier compare, and prove the shared
//! operand registers arrive unchanged across the open interval.
//!
//! The shadow must precede the compare in its own block. On every path
//! reaching the compare — a block that cannot reach itself admits each
//! arrival as its first — the flag state the compare would publish is
//! already the state the shadow left: identical kind and operand registers
//! compute identical flags, the per-unit last-event scan rules out any
//! intervening flag write, and the interval's operand audit rules out a
//! same-named register holding a different value. A block that lies on a
//! cycle refuses outright: a re-entering traversal's last flag event is the
//! compare's own earlier execution, which the single-shadow equivalence
//! cannot represent.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    VirtualRegisterId,
};

use super::super::DeadCompareError;
use super::super::admission::{compare_operand_arity, shifted_boundary_settlements, successors};
use super::RedundantCompareError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub compare_index: usize,
}

/// The plain-`Use` operand registers of a canonical flag publisher, or
/// none: the compare kinds are admitted only in their emitted shape —
/// canonical operand positions, no implicit uses, and a nonempty published
/// surface — so pairwise register equality really is value equality.
fn canonical_compare_operands(instruction: &SelectedInstruction) -> Option<Vec<VirtualRegisterId>> {
    if compare_operand_arity(instruction.kind)? != instruction.operands.len()
        || !instruction.implicit_uses.is_empty()
        || instruction
            .implicit_defs
            .iter()
            .chain(instruction.clobbers.iter())
            .next()
            .is_none()
        || instruction
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
        return None;
    }
    Some(
        instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .collect(),
    )
}

/// Whether `block` can reach itself through its successor edges: a path
/// leaving the block and arriving again makes a second traversal whose flag
/// observations the in-block scan cannot describe.
fn on_cycle(function: &SelectedFunction, block_index: usize) -> bool {
    let mut visited = BTreeSet::new();
    let mut frontier = Vec::new();
    for successor in successors(&function.blocks[block_index]) {
        if let Some(target) = function
            .blocks
            .iter()
            .position(|block| block.id == successor.block)
        {
            frontier.push(target);
        }
    }
    while let Some(current) = frontier.pop() {
        if current == block_index {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        for successor in successors(&function.blocks[current]) {
            if let Some(target) = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
            {
                frontier.push(target);
            }
        }
    }
    false
}

/// Every operand register the two compares share must keep its value across
/// the open interval: any write — a `Def` or `UseDef` operand, or an
/// early-clobbered operand — could give the compare a different operand
/// value than the shadow computed flags from. Plain reads are harmless.
fn audit_stable_operands(
    block: &SelectedBlock,
    shadow_index: usize,
    compare_index: usize,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<(), RedundantCompareError> {
    for instruction in &block.instructions[shadow_index + 1..compare_index] {
        if instruction.operands.iter().any(|operand| {
            registers.contains(&operand.virtual_register)
                && (operand.access != RegisterOperandAccess::Use || operand.early_clobber)
        }) {
            return Err(RedundantCompareError::UnsupportedUse);
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
) -> Result<Admission<'source>, RedundantCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RedundantCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RedundantCompareError::SourceMismatch)?;
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
        .ok_or(RedundantCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in the
    // emitted flag-publisher shape: every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently drop,
    // and a nonempty published surface.
    let operand_registers: BTreeSet<VirtualRegisterId> =
        canonical_compare_operands(compare_instruction)
            .ok_or(RedundantCompareError::UnsupportedInstruction)?
            .into_iter()
            .collect();
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees would
    // change what removing the instruction means.
    let arity = compare_instruction.operands.len();
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(RedundantCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(RedundantCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(RedundantCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(RedundantCompareError::ConstraintMismatch);
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
        return Err(RedundantCompareError::UnsupportedInstruction);
    }
    // A block that can reach itself refuses: a re-entering traversal's last
    // flag event is the compare's own earlier execution — or a later
    // in-block event — never the resolved shadow, so the in-block
    // equivalence argument does not describe it.
    if on_cycle(function, block_index) {
        return Err(RedundantCompareError::UnsupportedUse);
    }
    // Every published unit's last in-block flag event must be the same
    // position: the shadow. A unit untouched earlier in the block reaches
    // the compare through predecessor state the in-block bound does not
    // admit; two different positions refuse.
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    let mut shadow_index = None;
    for &unit in &units {
        let event = block.instructions[..compare_index]
            .iter()
            .rposition(|instruction| {
                instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit)
            })
            .ok_or(RedundantCompareError::UnsupportedUse)?;
        if shadow_index.is_some_and(|index| index != event) {
            return Err(RedundantCompareError::UnsupportedUse);
        }
        shadow_index = Some(event);
    }
    let shadow_index = shadow_index.ok_or(RedundantCompareError::UnsupportedUse)?;
    let shadow = &block.instructions[shadow_index];
    // The shadow must be a flag-equivalent compare: canonical shape, the
    // same kind — immediate payload included — and the same operand
    // registers in the same positions, so its published values are the
    // compare's. Positions matter: a swapped pair is a different
    // subtraction. Each published unit must also carry the same role there:
    // the shadow defines what the compare defines and clobbers what it
    // clobbers; a definition shadowing as a clobber would leave readers an
    // unspecified unit where the compare published real flags.
    let shadow_registers =
        canonical_compare_operands(shadow).ok_or(RedundantCompareError::UnsupportedUse)?;
    let compare_registers: Vec<VirtualRegisterId> = compare_instruction
        .operands
        .iter()
        .map(|operand| operand.virtual_register)
        .collect();
    if shadow.kind != compare_instruction.kind || shadow_registers != compare_registers {
        return Err(RedundantCompareError::UnsupportedUse);
    }
    for &unit in &units {
        if compare_instruction.implicit_defs.contains(&unit) != shadow.implicit_defs.contains(&unit)
        {
            return Err(RedundantCompareError::UnsupportedUse);
        }
    }
    // The operand registers must arrive unchanged: no write to any of them
    // in the open interval between the shadow and the compare.
    audit_stable_operands(block, shadow_index, compare_index, &operand_registers)?;
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    // The per-unit last-event scans and the interval's operand audit each
    // stay inside the compare's block, so one function traversal apiece
    // bounds them; the cycle check walks each block and successor edge once.
    let edge_count = function
        .blocks
        .iter()
        .map(|block| successors(block).count())
        .sum::<usize>();
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
        .and_then(|total| {
            total.checked_add(
                units
                    .len()
                    .checked_mul(function_scan)?
                    .checked_add(function_scan)?
                    .checked_add(function.blocks.len())?
                    .checked_add(edge_count)?,
            )
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RedundantCompareError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RedundantCompareError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        compare_index,
    })
}

/// The one-function transformation shared by proposal and replay: remove
/// the compare and shift boundary settlements over the removed ordinal —
/// the same remap the dead family applies. Both sides compute it from the
/// source, never from each other.
pub(super) fn apply(
    admitted: &Admission<'_>,
    function: &mut SelectedFunction,
) -> Result<(), RedundantCompareError> {
    let block = function
        .blocks
        .get_mut(admitted.block_index)
        .ok_or(RedundantCompareError::SourceMismatch)?;
    if block.id != admitted.block {
        return Err(RedundantCompareError::SourceMismatch);
    }
    function.boundary_settlements =
        shifted_boundary_settlements(admitted.function, admitted.block, admitted.compare_index)
            .map_err(|error| match error {
                DeadCompareError::IdentityOverflow => RedundantCompareError::IdentityOverflow,
                _ => RedundantCompareError::SourceMismatch,
            })?;
    block.instructions.remove(admitted.compare_index);
    Ok(())
}
