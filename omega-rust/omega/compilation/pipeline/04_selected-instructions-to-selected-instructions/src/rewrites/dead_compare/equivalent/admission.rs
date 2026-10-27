//! Admission for equivalent-compare removal: the shared flag-publisher
//! compare-removal walk under the value-equivalence shadow rule.
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
//! materialization stays for its other readers, so a later register-form
//! compare computes the subtraction the folded shadow already published.
//!
//! The walk itself — the cone, the reaching-set resolution, the per-unit
//! backward audit and its budget charge — is the shared
//! [`super::super::admission::admit_shadowed_compare`]; what this family
//! supplies is the victim's own subtraction sides and the site-by-site proof
//! that a shadow's sides carry the same bits.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    VirtualRegisterId,
};

use super::super::admission::{
    Admission, ShadowRefusals, ShadowVictim, admit_shadowed_compare, canonical_compare_operands,
};
use super::EquivalentCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{immediate_bits, materialized_bits};

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
    admit_shadowed_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        ShadowRefusals {
            source: EquivalentCompareError::SourceMismatch,
            unsupported_instruction: EquivalentCompareError::UnsupportedInstruction,
            constraint: EquivalentCompareError::ConstraintMismatch,
            unsupported_use: EquivalentCompareError::UnsupportedUse,
            overflow: EquivalentCompareError::IdentityOverflow,
            budget: EquivalentCompareError::WorkBudgetExceeded,
        },
        // The victim's own read: its subtraction sides, under the same
        // emitted-shape and encoding rules a candidate shadow must satisfy.
        |instruction| {
            let sides = compare_sides(instruction)?;
            let registers = canonical_compare_operands(instruction)?;
            Some(ShadowVictim {
                registers,
                read: sides,
            })
        },
        // A defined unit's reaching site must define the unit and compute the
        // same subtraction; the registers the two compares share by identity
        // are the ones the backward audit then holds stable.
        |function, _victim_instruction, victim, event, audited| {
            let Some(site_sides) = compare_sides(event) else {
                return false;
            };
            sides_equal(function, victim.read, site_sides, audited)
        },
        // The equivalent family's per-unit term: the reaching walk's base plus
        // the per-site equivalence checks' unique-producer scans.
        |cone| {
            cone.per_unit_base
                .checked_add(
                    cone.elements
                        .checked_mul(cone.function_scan)
                        .and_then(|product| product.checked_mul(4))
                        .ok_or(EquivalentCompareError::IdentityOverflow)?,
                )
                .ok_or(EquivalentCompareError::IdentityOverflow)
        },
    )
}
