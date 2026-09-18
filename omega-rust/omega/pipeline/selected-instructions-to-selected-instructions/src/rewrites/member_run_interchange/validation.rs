use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    MemberRunInterchangeError, MemberRunInterchangeReceipt, ValidatedMemberRunInterchange,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted member, run, and window from the source, the touched block's
/// window must equal exactly the later side, the interior, and the
/// earlier side in that order — the member and the run exchanged, the
/// interior between them keeping its relative order — and splicing the
/// source window back must restore the complete source by content: every
/// instruction before and after the window, every other instruction,
/// register, roster row, call, settlement, and function included.
pub fn validate_member_run_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedMemberRunInterchange, MemberRunInterchangeError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        run_first,
        run_last,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.block_index))
        .ok_or(MemberRunInterchangeError::ReplayMismatch)?;
    let first = admitted.member_index.min(admitted.run_first_index);
    let last = admitted.member_index.max(admitted.run_last_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(MemberRunInterchangeError::ReplayMismatch)?;
    let run_len = admitted.run_last_index - admitted.run_first_index + 1;
    let member_earlier = admitted.member_index < admitted.run_first_index;
    let earlier_len = if member_earlier { 1 } else { run_len };
    let later_len = if member_earlier { run_len } else { 1 };
    let interior_len = source_window.len() - earlier_len - later_len;
    // The proposal's window must be the source window's own instructions
    // rearranged to later-side ++ interior ++ earlier-side — no other
    // member, order, or content.
    let expected: Vec<_> = source_window[earlier_len + interior_len..]
        .iter()
        .chain(&source_window[earlier_len..earlier_len + interior_len])
        .chain(&source_window[..earlier_len])
        .collect();
    if proposed_block
        .instructions
        .get(first..=last)
        .map(|window| window.iter().collect::<Vec<_>>())
        != Some(expected)
    {
        return Err(MemberRunInterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    if restored != *source.selected_plan() {
        return Err(MemberRunInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedMemberRunInterchange {
        receipt: MemberRunInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
