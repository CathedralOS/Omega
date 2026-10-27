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
use selected_instructions::{SelectedInstructionId, VirtualRegisterId};

use super::super::admission::{
    Admission, agree_compare_removal_row, audit_stable_paths, canonical_compare_operands,
};
use super::RedundantCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::member_position;
use crate::rewrites::condition_state::{
    adjacency, backward_cone, entry_index, instruction_at, reaching_events,
};

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RedundantCompareError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            RedundantCompareError::SourceMismatch
        })?;
    let (block_index, compare_index) =
        member_position(function, compare).ok_or(RedundantCompareError::SourceMismatch)?;
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
    // implicit surface the instruction carries, and nothing may bind the
    // removed instruction by identity.
    let arity = compare_instruction.operands.len();
    agree_compare_removal_row(
        function,
        compare,
        compare_instruction,
        arity,
        environment,
        || RedundantCompareError::ConstraintMismatch,
        || RedundantCompareError::UnsupportedInstruction,
    )?;
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
                RedundantCompareError::UnsupportedUse,
            )?;
        }
    }
    if super::measured_steps(plan, function, &successors, units.len())? > budget.validation_steps()
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
