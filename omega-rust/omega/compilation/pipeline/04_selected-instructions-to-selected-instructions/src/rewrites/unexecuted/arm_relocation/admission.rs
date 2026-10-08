//! Shared admission for arm relocation: locate the named `member` in one
//! block's body, require that block to be a conditional arm — a plain
//! source block whose every predecessor edge leaves one fork head ending
//! in a two-successor conditional branch — locate the named `destination`
//! in that head's body, and prove the move sound in both directions — the
//! crossed window independent (no register or condition-state hazard
//! between the member and any crossed position, no interference with a
//! crossed edge's register transports, no barrier, call, hosted effect,
//! or call-roster entry inside the window, and no boundary settlement
//! whose observed executed prefix changes) and every member-written
//! location dead on every path the member's new position adds it to.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::ArmRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::relocation_scan_steps;

pub(super) struct Admission {
    /// The member's own block: the branch's arm.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The fork head's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the head body: the
    /// member lands at this index. Naming the head's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, ArmRelocationError> {
    let derived =
        super::derive_arm_relocation(source, function_index, member, destination, environment)?;
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to find the arm's
    // predecessor edges; the window audit walks the member's surface
    // against each crossed position's; the dead-path audit rescans a
    // block's stream and edge surfaces only while its entry set grows —
    // at most once per member location per block. The arm's predecessor
    // walk needs no per-block term of its own.
    let steps = relocation_scan_steps(
        derived.plan,
        derived.function,
        derived.member_instruction,
        &derived.crossing,
        derived.edge_limit,
        &|_| 0,
    )
    .ok_or(ArmRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ArmRelocationError::IdentityOverflow,
        || ArmRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Admission {
        block_index: derived.block_index,
        member_index: derived.member_index,
        target_index: derived.target_index,
        landing_index: derived.landing_index,
    })
}
