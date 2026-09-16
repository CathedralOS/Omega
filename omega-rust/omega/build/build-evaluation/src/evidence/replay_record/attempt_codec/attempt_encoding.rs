//! Encoding one attempt.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::wire::{
    Encoder, access_tag, handle_kind_tag, metadata_kind_tag, observed_region_kind_tag,
    provider_tag, refusal_reason_tag, returned_path_completeness_tag, returned_path_kind_tag,
    root_tag,
};
use crate::{
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationObservationClass,
    BuildFilesystemOperationResult, BuildFilesystemScalarOperandValue,
};

pub(crate) fn encode_attempt(
    encoder: &mut Encoder,
    attempt: &BuildFilesystemOperationAttempt,
) -> Result<(), BuildFilesystemReplayRecordError> {
    encoder.u16(attempt.operation_tag());
    encoder.byte(provider_tag(attempt.provider()));
    encoder.byte(match attempt.observation_class() {
        BuildFilesystemOperationObservationClass::Receipted => 0,
        BuildFilesystemOperationObservationClass::Volatile => 1,
    });
    match attempt.result() {
        BuildFilesystemOperationResult::Scalar(value) => {
            encoder.byte(0);
            encoder.i64(value);
        }
        BuildFilesystemOperationResult::LogicalHandle(identity) => {
            encoder.byte(1);
            encoder.u64(identity.get());
        }
    }
    encoder.i32(attempt.post_error());

    encoder.count(attempt.scalar_operands().len())?;
    for operand in attempt.scalar_operands() {
        encoder.byte(operand.operand_ordinal());
        match operand.value() {
            BuildFilesystemScalarOperandValue::I32(value) => {
                encoder.byte(0);
                encoder.i32(value);
            }
            BuildFilesystemScalarOperandValue::U32(value) => {
                encoder.byte(1);
                encoder.u32(value);
            }
            BuildFilesystemScalarOperandValue::I64(value) => {
                encoder.byte(2);
                encoder.i64(value);
            }
            BuildFilesystemScalarOperandValue::U64(value) => {
                encoder.byte(3);
                encoder.u64(value);
            }
        }
    }

    encoder.count(attempt.byte_operands().len())?;
    for operand in attempt.byte_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.path_like_operands().len())?;
    for operand in attempt.path_like_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.rooted_path_operand_resolutions().len())?;
    for operand in attempt.rooted_path_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.byte(root_tag(operand.root()));
        encoder.bytes(operand.relative_path())?;
    }
    encoder.count(attempt.returned_paths().len())?;
    for returned in attempt.returned_paths() {
        encoder.byte(returned.operand_ordinal());
        encoder.byte(returned_path_kind_tag(returned.kind()));
        encoder.byte(returned_path_completeness_tag(returned.completeness()));
        encoder.bytes(returned.bytes())?;
    }
    encoder.count(attempt.observed_byte_regions().len())?;
    for region in attempt.observed_byte_regions() {
        encoder.byte(region.output_operand_ordinal());
        encoder.byte(observed_region_kind_tag(region.kind()));
        encoder.u64(region.offset());
        encoder.u64(region.length());
    }
    encoder.count(attempt.metadata_observations().len())?;
    for metadata in attempt.metadata_observations() {
        encoder.byte(metadata.output_operand_ordinal());
        encoder.byte(metadata_kind_tag(metadata.kind()));
        encoder.u64(metadata.device());
        encoder.u32(metadata.mode());
        encoder.u64(metadata.link_count());
        encoder.u64(metadata.inode());
        encoder.u32(metadata.user());
        encoder.u32(metadata.group());
        encoder.u64(metadata.referenced_device());
        encoder.i64(metadata.access_time());
        encoder.i64(metadata.modification_time());
        encoder.i64(metadata.change_time());
        encoder.i64(metadata.birth_time());
        encoder.i64(metadata.size());
        encoder.u64(metadata.blocks_512());
        encoder.u64(metadata.preferred_block_size());
    }
    encoder.count(attempt.mutable_byte_operand_resolutions().len())?;
    for operand in attempt.mutable_byte_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.mutable_i64_operand_resolutions().len())?;
    for operand in attempt.mutable_i64_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.i64(operand.value());
    }
    encoder.count(attempt.mutable_byte_operands().len())?;
    for operand in attempt.mutable_byte_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.pre_bytes())?;
        encoder.bytes(operand.post_bytes())?;
    }
    encoder.count(attempt.mutable_i64_operands().len())?;
    for operand in attempt.mutable_i64_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.i64(operand.pre_value());
        encoder.i64(operand.post_value());
    }
    encoder.count(attempt.authorized_paths().len())?;
    for path in attempt.authorized_paths() {
        encoder.byte(path.operand_ordinal());
        encoder.byte(access_tag(path.access()));
        encoder.byte(root_tag(path.root()));
        encoder.bytes(path.relative_path())?;
    }
    encoder.count(attempt.logical_handle_inputs().len())?;
    for input in attempt.logical_handle_inputs() {
        encoder.byte(input.operand_ordinal());
        encoder.byte(handle_kind_tag(input.kind()));
        match input.resolution() {
            BuildFilesystemLogicalHandleInputResolution::Resolved(identity) => {
                encoder.byte(0);
                encoder.u64(identity.get());
            }
            BuildFilesystemLogicalHandleInputResolution::Null => encoder.byte(1),
            BuildFilesystemLogicalHandleInputResolution::Unknown => encoder.byte(2),
        }
    }
    match attempt.logical_handle_output() {
        None => encoder.byte(0),
        Some(output) => {
            encoder.byte(1);
            encoder.byte(handle_kind_tag(output.kind()));
            encoder.u64(output.identity().get());
            match output.source() {
                BuildFilesystemLogicalHandleOutputSource::Created => encoder.byte(0),
                BuildFilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                    encoder.byte(1);
                    encoder.u64(identity.get());
                }
                BuildFilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                    encoder.byte(2);
                    encoder.u64(identity.get());
                }
            }
        }
    }
    encoder.count(attempt.retired_logical_handles().len())?;
    for identity in attempt.retired_logical_handles() {
        encoder.u64(identity.get());
    }
    encoder.count(attempt.grant_refusals().len())?;
    for refusal in attempt.grant_refusals() {
        encoder.byte(refusal.operand_ordinal());
        encoder.byte(access_tag(refusal.access()));
        encoder.byte(refusal_reason_tag(refusal.reason()));
    }
    Ok(())
}
