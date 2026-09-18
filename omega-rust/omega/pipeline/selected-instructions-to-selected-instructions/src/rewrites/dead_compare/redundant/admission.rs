//! Shared admission for redundant compare removal: locate the named
//! compare, confirm its canonical flag-publisher shape against the target's
//! own constraint row, resolve every published unit's reaching flag events
//! to flag-equivalent compares, and prove the shared operand registers
//! arrive unchanged along every path between a shadow and the compare.
//!
//! A shadow may sit in the compare's own block or behind an edge: the
//! shared condition-state walk resolves each published unit's reaching
//! events — the last in-block event when one precedes the compare,
//! otherwise the least-fixpoint entry set over the predecessor cone — and
//! every site the set resolves to must leave the unit in the state the
//! compare republishes. A unit the compare defines needs each reaching
//! site to be a flag-equivalent compare that defines it; a unit the
//! compare only clobbers needs each site to leave it unspecified. The
//! compare's own earlier execution can serve as a cyclic arrival's site —
//! it is trivially flag-equivalent — so a block on a cycle admits whenever
//! the operand audit also passes.
//!
//! The operand audit runs backward from the compare through the
//! predecessor cone, once per defined unit, and stops at that unit's own
//! resolved shadows: a position is exposed for the unit exactly when a
//! path carries it to the compare with none of the unit's reaching sites
//! in between, and no exposed position may write a shared operand
//! register — not an instruction operand, and not an edge transport's
//! parameter. Another unit's shadow does not block the walk: it
//! republishes its own unit only, so a write it crosses still stands
//! between this unit's last site and the compare. Writes before an
//! intervening same-unit shadow are rescued by the identical
//! republication, so they never reach the exposed set.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedValueTransport, VirtualRegisterId,
};

use super::super::DeadCompareError;
use super::super::admission::{compare_operand_arity, shifted_boundary_settlements};
use super::RedundantCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::condition_state::{
    EventSite, adjacency, backward_cone, entry_index, instruction_at, reaching_events,
};

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

/// Every operand register the compare reads must keep its value between
/// one unit's last shadow execution on a path and the arrival at the
/// compare. The exposed positions are exactly those that can reach the
/// compare backward without crossing a site in that unit's reaching set —
/// a shadow's execution resets the unit's binding segment because it
/// republishes the identical flag value, so writes before it are rescued,
/// while another unit's shadow republishes only its own unit and cannot
/// stand in. The walk marks each visited `(block, position)` once: body
/// positions expand to the position before them, a block's first position
/// expands to its predecessor blocks' terminator positions, and each
/// crossed edge's register transports face the same audit — an edge
/// parameter the edge defines for its target is a write of the target's
/// register.
fn audit_stable_paths(
    function: &SelectedFunction,
    predecessors: &[Vec<usize>],
    block_index: usize,
    compare_index: usize,
    unit_sites: &BTreeSet<EventSite>,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<(), RedundantCompareError> {
    let compare_site = (block_index, compare_index);
    let mut visited = BTreeSet::new();
    let mut frontier = vec![compare_site];
    while let Some(site @ (block, position)) = frontier.pop() {
        if !visited.insert(site) {
            continue;
        }
        // The compare's own position seeds the walk and ends cyclic
        // encounters; every other resolved shadow for the unit blocks the
        // exposed interval from reaching behind it.
        if site != compare_site && unit_sites.contains(&site) {
            continue;
        }
        let instruction = instruction_at(function, site);
        if instruction.operands.iter().any(|operand| {
            registers.contains(&operand.virtual_register)
                && (operand.access != RegisterOperandAccess::Use || operand.early_clobber)
        }) {
            return Err(RedundantCompareError::UnsupportedUse);
        }
        if position > 0 {
            frontier.push((block, position - 1));
            continue;
        }
        for &predecessor in &predecessors[block] {
            for successor in terminator_successors(&function.blocks[predecessor].terminator) {
                if successor.block != function.blocks[block].id {
                    continue;
                }
                for binding in &successor.bindings {
                    if let SelectedValueTransport::Registers { parameter, .. } = binding.transport
                        && registers.contains(&parameter)
                    {
                        return Err(RedundantCompareError::UnsupportedUse);
                    }
                }
                if let Some(case) = &successor.structural_case {
                    for payload in &case.payloads {
                        let parameter = match payload.transport {
                            SelectedCasePayloadTransport::Unused => continue,
                            SelectedCasePayloadTransport::Unmaterialized { parameter }
                            | SelectedCasePayloadTransport::Registers { parameter, .. } => {
                                parameter
                            }
                        };
                        if registers.contains(&parameter) {
                            return Err(RedundantCompareError::UnsupportedUse);
                        }
                    }
                }
            }
            frontier.push((predecessor, function.blocks[predecessor].instructions.len()));
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
    let compare_registers = canonical_compare_operands(compare_instruction)
        .ok_or(RedundantCompareError::UnsupportedInstruction)?;
    let operand_registers: BTreeSet<VirtualRegisterId> =
        compare_registers.iter().copied().collect();
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
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // compare's block — only their entry sets can feed the reaching events.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(RedundantCompareError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every unit the compare publishes — defined or clobbered — must reach
    // the compare carrying the state the compare republishes. The shared
    // walk resolves each unit's reaching set: the last in-block event when
    // one precedes the compare, otherwise every site a path may last have
    // observed across edges. A defined unit needs every site to be a
    // flag-equivalent compare — canonical shape, identical kind including
    // any immediate payload, and the same operand registers in the same
    // positions — that defines rather than clobbers the unit, and its own
    // reaching set then bounds the operand audit: no position exposed
    // between a unit site and the compare may write a shared register. A
    // clobbered unit needs every site to leave it unspecified: a clobber,
    // never a definition whose flags the removal would expose.
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
        .map_err(|_| RedundantCompareError::UnsupportedUse)?;
        let mut unit_sites = BTreeSet::new();
        for site in sites {
            let event = instruction_at(function, site);
            if defined {
                let site_registers = canonical_compare_operands(event)
                    .ok_or(RedundantCompareError::UnsupportedUse)?;
                if event.kind != compare_instruction.kind
                    || site_registers != compare_registers
                    || !event.implicit_defs.contains(&unit)
                    || event.clobbers.contains(&unit)
                {
                    return Err(RedundantCompareError::UnsupportedUse);
                }
                unit_sites.insert(site);
            } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit) {
                return Err(RedundantCompareError::UnsupportedUse);
            }
        }
        if defined {
            audit_stable_paths(
                function,
                &predecessors,
                block_index,
                compare_index,
                &unit_sites,
                &operand_registers,
            )?;
        }
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    // The shared walk's setup resolves every edge's target index, fills
    // the predecessor lists, and marks the backward cone. Each unit's
    // resolution scans the compare block's prefix and — on an in-block
    // miss — every block's stream for its last event, then propagates
    // entry sets: a block requeues only while its set grows, a set holds
    // at most one element per event site plus the unknown marker, and
    // each pop visits its out-edges at a bounded union cost. The
    // operand audit visits each stream position at most once and reads
    // each crossed edge's transport surface once.
    let elements = function_scan
        .checked_add(1)
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
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
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    // The backward operand audit runs once per published unit: each walk
    // visits a stream position at most once and reads each crossed edge's
    // transport surface at most once.
    let audit = units
        .len()
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .ok_or(RedundantCompareError::IdentityOverflow)?;
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
