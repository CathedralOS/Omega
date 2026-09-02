//! Version admission, legacy identity replay, and authenticated payload decoding.

use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::super::identity::fixed_view_copy_identity_v3_legacy;
use super::envelope::{
    v5_identity, v6_identity, v7_identity, v8_identity, v9_identity, v10_identity,
};
use super::primitives::Cursor;
use super::{
    FixedViewCopyDecodeError, FixedViewCopyPlan, LEGACY_V4_VERSION, LEGACY_V5_VERSION,
    LEGACY_V6_VERSION, LEGACY_V7_VERSION, LEGACY_V8_VERSION, LEGACY_V9_VERSION, MAGIC, VERSION,
    content,
};

pub(super) fn decode(encoded: &[u8]) -> Result<FixedViewCopyPlan, FixedViewCopyDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(FixedViewCopyDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if !matches!(
        version,
        LEGACY_V4_VERSION
            | LEGACY_V5_VERSION
            | LEGACY_V6_VERSION
            | LEGACY_V7_VERSION
            | LEGACY_V8_VERSION
            | LEGACY_V9_VERSION
            | VERSION
    ) {
        return Err(FixedViewCopyDecodeError::UnsupportedVersion(version));
    }
    let identity: [u8; 32] = cursor.array()?;
    let content_offset = cursor.offset;
    let decoded = match version {
        LEGACY_V4_VERSION => content::decode_v4(&mut cursor)?,
        LEGACY_V5_VERSION => content::decode_v5(&mut cursor)?,
        LEGACY_V6_VERSION | LEGACY_V7_VERSION | LEGACY_V8_VERSION | LEGACY_V9_VERSION | VERSION => {
            content::decode_v6(&mut cursor)?
        }
        _ => unreachable!("version admission is exhaustive"),
    };
    if cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    if matches!(
        version,
        LEGACY_V6_VERSION | LEGACY_V7_VERSION | LEGACY_V8_VERSION | LEGACY_V9_VERSION | VERSION
    ) && !decoded.transformed_payload_matches
    {
        return Err(FixedViewCopyDecodeError::TransformedPayloadMismatch);
    }
    let transformed = match version {
        LEGACY_V4_VERSION | LEGACY_V5_VERSION => {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v11_legacy(
                &decoded.plan.transformed,
            )
        }
        LEGACY_V6_VERSION => {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v13_legacy(
                &decoded.plan.transformed,
            )
        }
        LEGACY_V7_VERSION => {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v14_legacy(
                &decoded.plan.transformed,
            )
        }
        LEGACY_V8_VERSION => {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v15_legacy(
                &decoded.plan.transformed,
            )
        }
        LEGACY_V9_VERSION => {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v16_legacy(
                &decoded.plan.transformed,
            )
        }
        VERSION => selected_instruction_plan_identity(&decoded.plan.transformed),
        _ => unreachable!("version admission is exhaustive"),
    };
    if transformed != decoded.expected_transformed {
        return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
    }
    let actual_identity = match version {
        LEGACY_V4_VERSION => fixed_view_copy_identity_v3_legacy(&decoded.plan).bytes(),
        LEGACY_V5_VERSION => v5_identity(&decoded.plan, &encoded[content_offset..]),
        LEGACY_V6_VERSION => v6_identity(&decoded.plan, &encoded[content_offset..]),
        LEGACY_V7_VERSION => v7_identity(&decoded.plan, &encoded[content_offset..]),
        LEGACY_V8_VERSION => v8_identity(&decoded.plan, &encoded[content_offset..]),
        LEGACY_V9_VERSION => v9_identity(&decoded.plan, &encoded[content_offset..]),
        VERSION => v10_identity(&decoded.plan, &encoded[content_offset..]),
        _ => unreachable!("version admission is exhaustive"),
    };
    if actual_identity != identity {
        return Err(FixedViewCopyDecodeError::IdentityMismatch);
    }
    if version != VERSION && contains_i64_less_than(&decoded.plan.transformed) {
        return Err(FixedViewCopyDecodeError::UnknownInstructionKind(13));
    }
    Ok(decoded.plan)
}

fn contains_i64_less_than(plan: &omega_selected_instructions::SelectedInstructionPlan) -> bool {
    plan.functions.iter().flat_map(|function| &function.blocks).any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(instruction.kind, omega_selected_instructions::SelectedInstructionKind::ConditionalBranchI64LessThan)
        }) || matches!(
            block.terminator,
            omega_selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan { .. }
        )
    })
}
