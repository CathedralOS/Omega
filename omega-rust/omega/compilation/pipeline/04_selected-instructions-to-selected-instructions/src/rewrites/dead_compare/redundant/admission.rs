//! Admission for redundant compare removal: the shared flag-publisher
//! compare-removal walk under the strict shadow rule.
//!
//! A shadow may sit in the compare's own block or behind an edge: the shared
//! condition-state walk resolves each published unit's reaching events — the
//! last in-block event when one precedes the compare, otherwise the
//! least-fixpoint entry set over the predecessor cone — and every site the
//! set resolves to must leave the unit in the state the compare republishes.
//! A unit the compare defines needs each reaching site to be a flag-equivalent
//! compare that defines it; a unit the compare only clobbers needs each site
//! to leave it unspecified. The compare's own earlier execution can serve as a
//! cyclic arrival's site — it is trivially flag-equivalent — so a block on a
//! cycle admits whenever the operand audit also passes.
//!
//! The strict rule is the whole difference from the equivalent family: a
//! shadow must carry the compare's own kind — immediate payload included —
//! and read the same operand registers in the same positions. The registers
//! the two compares then share by identity are exactly the compare's own
//! canonical operands, so the rule records every one of them for the shared
//! backward audit, which runs from the compare through the predecessor cone
//! once per defined unit and stops at that unit's own resolved shadows: a
//! position is exposed for the unit exactly when a path carries it to the
//! compare with none of the unit's reaching sites in between, and no exposed
//! position may write a shared operand register — not an instruction operand,
//! and not an edge transport's parameter. Another unit's shadow does not
//! block the walk: it republishes its own unit only, so a write it crosses
//! still stands between this unit's last site and the compare. Writes before
//! an intervening same-unit shadow are rescued by the identical
//! republication, so they never reach the exposed set.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::super::admission::{
    Admission, ShadowRefusals, ShadowVictim, admit_shadowed_compare, canonical_compare_operands,
};
use super::RedundantCompareError;
use crate::ValidatedSelectedAnalysis;

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RedundantCompareError> {
    admit_shadowed_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        ShadowRefusals {
            source: RedundantCompareError::SourceMismatch,
            unsupported_instruction: RedundantCompareError::UnsupportedInstruction,
            constraint: RedundantCompareError::ConstraintMismatch,
            unsupported_use: RedundantCompareError::UnsupportedUse,
            overflow: RedundantCompareError::IdentityOverflow,
            budget: RedundantCompareError::WorkBudgetExceeded,
        },
        // The strict family's own read of the victim: the canonical
        // flag-publisher shape alone — the same kind and the same operand
        // registers are what a reaching site must reproduce.
        |instruction| {
            canonical_compare_operands(instruction).map(|registers| ShadowVictim {
                registers,
                read: (),
            })
        },
        // A defined unit's reaching site must be a flag-equivalent compare:
        // the same kind — immediate payload included — reading the same
        // operand registers in the same positions.
        |_function, victim_instruction, victim, event, audited| {
            let Some(site_registers) = canonical_compare_operands(event) else {
                return false;
            };
            if event.kind != victim_instruction.kind || site_registers != victim.registers {
                return false;
            }
            audited.extend(site_registers.iter().copied());
            true
        },
        // The strict family's per-unit term: the reaching walk's own base;
        // the shared cone audit supplies the rest of the contract.
        |cone| Ok(cone.per_unit_base),
    )
}
