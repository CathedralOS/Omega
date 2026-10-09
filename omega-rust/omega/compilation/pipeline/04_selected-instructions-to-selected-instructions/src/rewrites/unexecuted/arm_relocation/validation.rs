//! Independent validation of the arm relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! hoist's legality from the source records — the named member's arm
//! block (a plain source block whose every predecessor edge leaves one
//! fork head ending in a two-successor conditional branch), the named
//! destination's landing index in that head's body, the member's
//! hoistable kind, the crossed window's independence (no register or
//! condition-state hazard between the member and any crossed position,
//! no interference with a crossed edge's register transports, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes),
//! and the dead-path audit proving every member-written location dead on
//! every path the new position adds it to — then requires the proposal
//! to place exactly the member's instruction on the derived landing
//! index and restores the source by content: moving the member back must
//! reproduce every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge. A
//! producer admission error therefore fails validation even when the
//! proposal is exactly what that producer emitted.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionPlan, terminator_instruction,
};

use super::{ArmRelocationError, ArmRelocationReceipt, ValidatedArmRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
use crate::rewrites::window_hazards::{block_scan_surface, register_writes};

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted member's arm and index, the fork head, and the
/// landing index the walk proves. It shares no state with the producer's
/// `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    target_index: usize,
    landing_index: usize,
}

/// Re-derive the relocation's legality from the source records, without
/// the producer's `admission` routine: the shared derivation locates the
/// member's arm and head and proves the window, the validator's own step
/// accounting charges the audit, and a legality error surfaces here even
/// when the proposal matches the edit the producer emitted.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ArmRelocationError> {
    let derived =
        super::derive_arm_relocation(source, function_index, member, destination, environment)?;
    let plan = derived.plan;
    let function = derived.function;
    let member_instruction = derived.member_instruction;
    let crossing = &derived.crossing;
    let edge_limit = derived.edge_limit;
    let member_locations = register_writes(member_instruction).count()
        + member_instruction.implicit_defs.len()
        + member_instruction.clobbers.len();
    let block_scan = block_scan_surface(function);
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                candidate
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(
                        &candidate.terminator,
                    )))
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| {
            crate::rewrites::window_hazards::crossed_charge(
                total,
                function,
                member_instruction,
                crossing,
                edge_limit,
                block_scan,
                member_locations,
            )
        })
        .ok_or(ArmRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ArmRelocationError::IdentityOverflow,
        || ArmRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Reconstructed {
        function,
        block_index: derived.block_index,
        member_index: derived.member_index,
        target_index: derived.target_index,
        landing_index: derived.landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted member, head, and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// member's instruction on the landing index in the head block, and
/// moving it back must restore the complete source by content — every
/// crossed instruction, every other block and instruction, register,
/// roster row, call, settlement, and edge included.
pub(crate) fn validate_arm_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedArmRelocation, ArmRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_member = &reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.member_index];
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| block.instructions.get(reconstructed.landing_index))
        != Some(source_member)
    {
        return Err(ArmRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(ArmRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedArmRelocation,
        ArmRelocationReceipt,
        source,
        proposed
    ))
}
