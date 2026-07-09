//! Refuse FUSED operand-position SATURATING arithmetic (decision 17). A
//! `ValueOperand::Binary` encodes as the PLAIN integer op computed inline
//! (guard compares, nested write operand trees), so a Saturating operand read
//! the unclamped wide value (`sat_a + sat_b == 127` compared 150, silently
//! taking the wrong arm). Exact/Wrapping stay accepted: the plain op at the
//! compare/store byte width IS their semantics. TRAPPING is deliberately NOT
//! blocked yet: for every non-overflowing input the plain op is
//! value-identical (only the overflow trap itself is skipped), and the corpus
//! uses `in Trapping` pervasively as checked-arithmetic style -- blocking it
//! rejects green programs wholesale. The skipped-trap gap is tracked as a
//! pending canary (pending/arithmetic/runtime_trapping_guard_overflow) until
//! the operand-position trap sequence lands; the recorded domain keeps it
//! visible in the backend report (`Add/8 in Trapping`). The full-instruction
//! binary WRITE paths are unaffected (they key clamp/trap sequences off the
//! target's domain); this is only about arithmetic fused INSIDE an operand
//! tree, which has no clamping lowering yet.
//!
//! The sweep walks the OPERAND ARENA, not per-instruction operand fields, so a
//! new instruction kind embedding operand handles can never smuggle one past
//! the fence; instruction attribution below is best-effort (guard compares,
//! the only producer of TOP-LEVEL fused arithmetic today).

use crate::EmissionPlanningInput;
use crate::semantic_scope::proof_scope_suffix;
use omega_backend_report_types::EmissionBlocker;
use omega_core::arena::Arena;
use omega_core::arithmetic::ArithmeticDomain;
use omega_target_operations::{
    SelectedInstructionKind, StateGuardOperator, TargetValueOperand,
};

use super::{blocker, semantic_scope::state_name};

pub(super) fn collect_operand_domain_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let operands = &input.instructions.code.runtime_value_operands;
    for (handle, operand) in operands.iter() {
        let TargetValueOperand::Binary {
            operator,
            arithmetic_domain: domain @ ArithmeticDomain::Saturating,
            ..
        } = operand
        else {
            continue;
        };
        // Only the MAGNITUDE-GROWING operators diverge when fused: a plain
        // add/sub/mul computes the unclamped/untrapped wide value. Fused
        // divide/modulo run at the operand width and already match the
        // domain semantics for every representable result (signed MIN/-1
        // divide is the one edge: idiv TRAPS there, which IS the Trapping
        // behavior; the Saturating clamp face of that edge stays with the
        // store-position canary saturating_signed_divide_min_by_neg_one).
        // Comparisons/bitwise/shifts never carry an overflow to clamp.
        if !matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract | StateGuardOperator::Multiply
        ) {
            continue;
        }

        // Best-effort source attribution: a guard compare referencing this
        // operand (directly or through a nested tree) names the state.
        let location = input
            .instructions
            .code
            .instructions
            .iter()
            .find_map(|(_, instruction)| {
                let SelectedInstructionKind::CompareRuntimeValues { left, right, .. } =
                    instruction.kind
                else {
                    return None;
                };
                (operand_tree_contains(operands, left, handle)
                    || operand_tree_contains(operands, right, handle))
                .then(|| {
                    format!(
                        "{} statement {} transition guard",
                        state_name(input, instruction.source_key),
                        instruction.source_statement,
                    )
                })
            })
            .unwrap_or_else(|| "a runtime value operand".to_owned());

        blockers.insert(blocker(
            "operand domains",
            &format!(
                "{location} computes `{operator:?}` in the {domain:?} domain in OPERAND \
                 position, which has no native lowering yet -- the fused encoding would \
                 compute the plain operation (the unclamped value instead of \
                 saturating). Store the arithmetic result into a `{domain:?}`-typed \
                 local or field first, then use the stored value{}",
                proof_scope_suffix(input, input.entry_key),
            ),
        ));
    }
}

/// Whether `tree` is, or transitively contains, `needle` (operand trees are
/// small: a compare side is at most a short Binary/Convert chain).
fn operand_tree_contains(
    operands: &Arena<TargetValueOperand>,
    tree: omega_target_operations::TargetValueOperandHandle,
    needle: omega_target_operations::TargetValueOperandHandle,
) -> bool {
    if tree == needle {
        return true;
    }
    match operands.get(tree) {
        TargetValueOperand::Binary { left, right, .. } => {
            operand_tree_contains(operands, *left, needle)
                || operand_tree_contains(operands, *right, needle)
        }
        TargetValueOperand::Convert { source, .. } => {
            operand_tree_contains(operands, *source, needle)
        }
        TargetValueOperand::TextEqualsLiteral { place, .. } => {
            operand_tree_contains(operands, *place, needle)
        }
        _ => false,
    }
}
