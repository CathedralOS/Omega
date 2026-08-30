use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyDecodeError,
    FixedViewCopyPlan, FixedViewCopyPolicy, LiveRangeIdentity,
};

use super::{
    copy::{decode_copy, encode_copy},
    primitives::{Cursor, length},
    selected::{decode_selected_plan, encode_selected_plan},
};

pub(super) struct DecodedContent {
    pub(super) plan: FixedViewCopyPlan,
    pub(super) expected_transformed: SelectedInstructionPlanIdentity,
}

pub(super) fn encode(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(bytes, plan.copies.len());
    for copy in &plan.copies {
        encode_copy(bytes, copy);
    }
    bytes.extend_from_slice(
        &omega_target_operations_to_selected_instructions::selected_instruction_plan_identity(
            &plan.transformed,
        )
        .bytes(),
    );
    encode_selected_plan(bytes, &plan.transformed);
}

pub(super) fn decode(cursor: &mut Cursor<'_>) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let source_ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
    let source_legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        1 => FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        tag => return Err(FixedViewCopyDecodeError::UnknownPolicy(tag)),
    };
    let budget = omega_optimization_core::OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidBudget)?;
    let usage = omega_optimization_core::OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidUsage)?;
    let copy_count = cursor.length()?;
    let mut copies = Vec::with_capacity(copy_count.min(cursor.remaining()));
    for _ in 0..copy_count {
        copies.push(decode_copy(cursor)?);
    }
    let expected_transformed = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let transformed = decode_selected_plan(cursor)?;
    Ok(DecodedContent {
        plan: FixedViewCopyPlan {
            source_selected,
            source_ranges,
            source_legality,
            register_environment,
            allocator_availability,
            policy,
            budget,
            usage,
            copies,
            transformed,
        },
        expected_transformed,
    })
}
