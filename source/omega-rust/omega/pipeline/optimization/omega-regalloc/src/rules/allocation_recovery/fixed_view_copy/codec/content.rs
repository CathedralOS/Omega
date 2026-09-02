use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyDecodeError,
    FixedViewCopyPlan, FixedViewCopyPolicy, LiveRangeIdentity,
};

use super::{
    copy::{decode_copy, encode_copy},
    primitives::{Cursor, length},
    selected::{
        decode_selected_plan_v4, decode_selected_plan_v5, decode_selected_plan_v6,
        encode_selected_plan_v6,
    },
};

#[cfg(test)]
use super::selected::encode_selected_plan_v5;

pub(super) struct DecodedContent {
    pub(super) plan: FixedViewCopyPlan,
    pub(super) expected_transformed: SelectedInstructionPlanIdentity,
    pub(super) transformed_payload_matches: bool,
}

fn encode_prefix(
    bytes: &mut Vec<u8>,
    plan: &FixedViewCopyPlan,
    transformed: SelectedInstructionPlanIdentity,
) {
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
    bytes.extend_from_slice(&transformed.bytes());
}

#[cfg(test)]
pub(super) fn encode_v4(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    encode_prefix(
        bytes,
        plan,
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v11_legacy(
            &plan.transformed,
        ),
    );
    super::selected::encode_selected_plan_v4(bytes, &plan.transformed);
}

#[cfg(test)]
pub(super) fn encode_v5(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    encode_prefix(
        bytes,
        plan,
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v11_legacy(
            &plan.transformed,
        ),
    );
    encode_selected_plan_v5(bytes, &plan.transformed);
}

pub(super) fn encode_v6(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    encode_prefix(
        bytes,
        plan,
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity(
            &plan.transformed,
        ),
    );
    encode_selected_plan_v6(bytes, &plan.transformed);
}

#[cfg(test)]
pub(super) fn encode_legacy_v7(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    encode_prefix(
        bytes,
        plan,
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v14_legacy(
            &plan.transformed,
        ),
    );
    encode_selected_plan_v6(bytes, &plan.transformed);
}

#[cfg(test)]
pub(super) fn encode_legacy_v6(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    encode_prefix(
        bytes,
        plan,
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v13_legacy(
            &plan.transformed,
        ),
    );
    encode_selected_plan_v6(bytes, &plan.transformed);
}

struct DecodedPrefix {
    source_selected: SelectedInstructionPlanIdentity,
    source_ranges: LiveRangeIdentity,
    source_legality: AllocationLegalityIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    allocator_availability: AllocatorAvailabilityIdentity,
    policy: FixedViewCopyPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
    usage: omega_optimization_core::OptimizationWorkUsage,
    copies: Vec<crate::FixedViewCopy>,
    expected_transformed: SelectedInstructionPlanIdentity,
}

fn decode_prefix(cursor: &mut Cursor<'_>) -> Result<DecodedPrefix, FixedViewCopyDecodeError> {
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
    Ok(DecodedPrefix {
        source_selected,
        source_ranges,
        source_legality,
        register_environment,
        allocator_availability,
        policy,
        budget,
        usage,
        copies,
        expected_transformed,
    })
}

pub(super) fn decode_v4(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    let prefix = decode_prefix(cursor)?;
    finish(prefix, decode_selected_plan_v4(cursor)?, true)
}

pub(super) fn decode_v5(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    let prefix = decode_prefix(cursor)?;
    let decoded = decode_selected_plan_v5(cursor)?;
    finish(prefix, decoded.plan, decoded.payload_matches)
}

pub(super) fn decode_v6(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    let prefix = decode_prefix(cursor)?;
    let decoded = decode_selected_plan_v6(cursor)?;
    finish(prefix, decoded.plan, decoded.payload_matches)
}

fn finish(
    prefix: DecodedPrefix,
    transformed: omega_selected_instructions::SelectedInstructionPlan,
    transformed_payload_matches: bool,
) -> Result<DecodedContent, FixedViewCopyDecodeError> {
    Ok(DecodedContent {
        plan: FixedViewCopyPlan {
            source_selected: prefix.source_selected,
            source_ranges: prefix.source_ranges,
            source_legality: prefix.source_legality,
            register_environment: prefix.register_environment,
            allocator_availability: prefix.allocator_availability,
            policy: prefix.policy,
            budget: prefix.budget,
            usage: prefix.usage,
            copies: prefix.copies,
            transformed,
        },
        expected_transformed: prefix.expected_transformed,
        transformed_payload_matches,
    })
}
