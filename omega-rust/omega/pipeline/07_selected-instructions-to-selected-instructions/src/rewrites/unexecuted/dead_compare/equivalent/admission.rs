//! Shared admission for equivalent-compare removal: locate the named
//! compare, confirm its canonical flag-publisher shape, resolve every
//! published unit's reaching flag events to value-equivalent compare
//! shadows, and prove the operand registers the pair shares by identity
//! arrive unchanged along every path between a shadow and the compare.
//!
//! Value equivalence relaxes the strict family's form identity. The shadow
//! may be any of the three compare kinds — immediate payload included —
//! and may read different operand registers, provided each semantic side of
//! the subtraction coincides: by register identity, whose drift the
//! backward path audit then refuses, or by literal, where the register's
//! unique `MaterializeI64` producer pins the bits function-wide and no
//! interval audit is needed at all. The cross-form shadow is the shape the
//! selection folds leave behind: the compare pair rules rewrite
//! `CompareI64` into `CompareI64Immediate` or `CompareI64Zero` while the
//! materialization stays for its other readers, so a later register-form compare computes
//! the subtraction the folded shadow already published.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, VirtualRegisterId,
};

use super::super::DeadCompareError;
use super::super::admission::{
    audit_stable_paths, canonical_compare_operands, shifted_boundary_settlements,
};
use super::EquivalentCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::unexecuted::condition_state::{
    adjacency, backward_cone, entry_index, immediate_bits, instruction_at, materialized_bits,
    reaching_events,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub compare_index: usize,
}

/// The semantic right side a canonical compare subtracts: the second
/// register operand for `CompareI64`, or the literal the form encodes —
/// `CompareI64Zero` reads as `left - 0`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompareRight {
    Register(VirtualRegisterId),
    Literal(u64),
}

/// The `(left, right)` value pair whose subtraction a canonical flag
/// publisher's flag publication describes. The left side is always a
/// register operand; the right is a register for `CompareI64` and the
/// encoded literal for `CompareI64Immediate` and `CompareI64Zero`. An
/// instruction outside the emitted shape — or an immediate payload outside
/// the unsigned encoding the form admits — is not even a candidate.
fn compare_sides(instruction: &SelectedInstruction) -> Option<(VirtualRegisterId, CompareRight)> {
    let operands = canonical_compare_operands(instruction)?;
    match instruction.kind {
        SelectedInstructionKind::CompareI64 => {
            Some((operands[0], CompareRight::Register(operands[1])))
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => Some((
            operands[0],
            CompareRight::Literal(immediate_bits(immediate)?),
        )),
        SelectedInstructionKind::CompareI64Zero => Some((operands[0], CompareRight::Literal(0))),
        _ => None,
    }
}

/// The bit pattern one right side provably reads: a literal carries its own
/// bits, and a register resolves through its unique `MaterializeI64`
/// producer — a function-wide guarantee, so the pinned value holds wherever
/// the register could be read rather than at one site.
fn right_bits(function: &SelectedFunction, side: CompareRight) -> Option<u64> {
    match side {
        CompareRight::Register(register) => materialized_bits(function, register).ok(),
        CompareRight::Literal(bits) => Some(bits),
    }
}

/// Whether `shadow` computes the same subtraction `victim` does, side by
/// side in the compare's own direction. A side coincides by register
/// identity — the same virtual register on both, whose value could still
/// drift between shadow and compare, so it lands in `audited` for the
/// caller's path audit — or by literal: each side resolves to the same bit
/// pattern, a register through its unique `MaterializeI64` producer and an
/// encoded literal through its own bits. A side that cannot be proven equal
/// — unshared registers without matching materializations, or a register
/// against an unmatched literal — fails, and the failure is what keeps a
/// swapped operand pair a different publication.
fn sides_equal(
    function: &SelectedFunction,
    victim: (VirtualRegisterId, CompareRight),
    shadow: (VirtualRegisterId, CompareRight),
    audited: &mut BTreeSet<VirtualRegisterId>,
) -> bool {
    let left_equal = if victim.0 == shadow.0 {
        audited.insert(victim.0);
        true
    } else {
        matches!(
            (
                materialized_bits(function, victim.0).ok(),
                materialized_bits(function, shadow.0).ok()
            ),
            (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
        )
    };
    if !left_equal {
        return false;
    }
    match (victim.1, shadow.1) {
        (CompareRight::Register(victim_register), CompareRight::Register(shadow_register))
            if victim_register == shadow_register =>
        {
            audited.insert(victim_register);
            true
        }
        (victim_right, shadow_right) => matches!(
            (
                right_bits(function, victim_right),
                right_bits(function, shadow_right)
            ),
            (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
        ),
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, EquivalentCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EquivalentCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EquivalentCompareError::SourceMismatch)?;
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
        .ok_or(EquivalentCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in the
    // emitted flag-publisher shape — every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently drop,
    // and a nonempty published surface — with an immediate payload the form
    // actually encodes.
    let victim_sides =
        compare_sides(compare_instruction).ok_or(EquivalentCompareError::UnsupportedInstruction)?;
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees would
    // change what removing the instruction means.
    let arity = compare_instruction.operands.len();
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(EquivalentCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(EquivalentCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(EquivalentCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(EquivalentCompareError::ConstraintMismatch);
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
        return Err(EquivalentCompareError::UnsupportedInstruction);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // compare's block — only their entry sets can feed the reaching events.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(EquivalentCompareError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every unit the compare publishes — defined or clobbered — must reach
    // the compare carrying the state the compare republishes. The shared
    // walk resolves each unit's reaching set: the last in-block event when
    // one precedes the compare, otherwise every site a path may last have
    // observed across edges. A defined unit needs every site to be a
    // canonical compare that defines the unit and computes the same
    // subtraction — form identity is not required, only the coinciding
    // operand values — and the unit's site set then bounds the operand
    // audit: no position exposed between a unit site and the compare may
    // write a register the two compares share by identity. A clobbered
    // unit needs every site to leave it unspecified: a clobber, never a
    // definition whose flags the removal would expose.
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    for &unit in &units {
        let defined = compare_instruction.implicit_defs.contains(&unit);
        let sites = reaching_events(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            compare_index,
            unit,
        )
        .map_err(|_| EquivalentCompareError::UnsupportedUse)?;
        let mut unit_sites = BTreeSet::new();
        let mut audited = BTreeSet::new();
        for site in sites {
            let event = instruction_at(function, site);
            if defined {
                let site_sides =
                    compare_sides(event).ok_or(EquivalentCompareError::UnsupportedUse)?;
                if !event.implicit_defs.contains(&unit)
                    || event.clobbers.contains(&unit)
                    || !sides_equal(function, victim_sides, site_sides, &mut audited)
                {
                    return Err(EquivalentCompareError::UnsupportedUse);
                }
                unit_sites.insert(site);
            } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit) {
                return Err(EquivalentCompareError::UnsupportedUse);
            }
        }
        if defined {
            audit_stable_paths(
                function,
                &predecessors,
                block_index,
                compare_index,
                &unit_sites,
                &audited,
                EquivalentCompareError::UnsupportedUse,
            )?;
        }
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    // The shared walk's setup resolves every edge's target index, fills
    // the predecessor lists, and marks the backward cone. Each unit's
    // resolution scans the compare block's prefix and — on an in-block
    // miss — every block's stream for its last event, then propagates
    // entry sets: a block requeues only while its set grows, a set holds
    // at most one element per event site plus the unknown marker, and
    // each pop visits its out-edges at a bounded union cost. Each
    // resolved site's equivalence check runs the unique-producer audit a
    // bounded number of times, each a full function scan. The operand
    // audit visits each stream position at most once and reads each
    // crossed edge's transport surface once.
    let elements = function_scan
        .checked_add(1)
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| total.checked_add(elements.checked_mul(function_scan)?.checked_mul(4)?))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let transport_rows = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            terminator_successors(&block.terminator)
                .iter()
                .try_fold(total, |total, successor| {
                    total.checked_add(edge_surface(successor))
                })
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    // The backward operand audit runs once per published unit: each walk
    // visits a stream position at most once and reads each crossed edge's
    // transport surface at most once.
    let audit = units
        .len()
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
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
            total
                .checked_add(walk_setup)?
                .checked_add(units.len().checked_mul(per_unit)?)?
                .checked_add(audit)
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| EquivalentCompareError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(EquivalentCompareError::WorkBudgetExceeded);
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
) -> Result<(), EquivalentCompareError> {
    let block = function
        .blocks
        .get_mut(admitted.block_index)
        .ok_or(EquivalentCompareError::SourceMismatch)?;
    if block.id != admitted.block {
        return Err(EquivalentCompareError::SourceMismatch);
    }
    function.boundary_settlements =
        shifted_boundary_settlements(admitted.function, admitted.block, admitted.compare_index)
            .map_err(|error| match error {
                DeadCompareError::IdentityOverflow => EquivalentCompareError::IdentityOverflow,
                _ => EquivalentCompareError::SourceMismatch,
            })?;
    block.instructions.remove(admitted.compare_index);
    Ok(())
}
