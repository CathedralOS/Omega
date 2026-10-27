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
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    VirtualRegisterId,
};

use super::super::admission::{
    Admission, agree_compare_removal_row, audit_stable_paths, canonical_compare_operands,
};
use super::EquivalentCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    adjacency, backward_cone, entry_index, immediate_bits, instruction_at, materialized_bits,
    reaching_events,
};

/// The semantic right side a canonical compare subtracts: the second
/// register operand for `CompareI64`, or the literal the form encodes —
/// `CompareI64Zero` reads as `left - 0`.
use crate::rewrites::block_edges::member_position;
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
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            EquivalentCompareError::SourceMismatch
        })?;
    let (block_index, compare_index) =
        member_position(function, compare).ok_or(EquivalentCompareError::SourceMismatch)?;
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
    // implicit surface the instruction carries, and nothing may bind the
    // removed instruction by identity.
    let arity = compare_instruction.operands.len();
    agree_compare_removal_row(
        function,
        compare,
        compare_instruction,
        arity,
        environment,
        || EquivalentCompareError::ConstraintMismatch,
        || EquivalentCompareError::UnsupportedInstruction,
    )?;
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
    if super::measured_steps(plan, function, &successors, units.len())? > budget.validation_steps()
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
