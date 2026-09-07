//! Ordered continuation edges retain their own zero-byte cleanup records.

use super::function_affine_cleanup_codec::{
    decode_unit_affine_cleanup, encode_unit_affine_cleanup,
};
use super::{InstallationError, Reader, push_u32, push_u64};
use machine_code::UnitContinuationRecord;
use semantic_vocabulary::BlockId;

pub(super) fn encode_unit_continuations(
    bytes: &mut Vec<u8>,
    continuations: &[UnitContinuationRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(continuations.len())
            .map_err(|_| InstallationError::TooManyStructuralReturnCleanups)?,
    );
    for continuation in continuations {
        push_u64(
            bytes,
            u64::try_from(continuation.operation_ordinal)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(continuation.successor_operation_ordinal)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u64(bytes, continuation.source_block.get());
        push_u64(bytes, continuation.target_block.get());
        encode_unit_affine_cleanup(bytes, &continuation.cleanup)?;
    }
    Ok(())
}

pub(super) fn decode_unit_continuations(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitContinuationRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralReturnCleanups)?;
    if count > reader.remaining() / 32 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut continuations = Vec::with_capacity(count);
    for _ in 0..count {
        continuations.push(UnitContinuationRecord {
            operation_ordinal: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
            successor_operation_ordinal: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
            source_block: BlockId::new(reader.u64()?).ok_or(
                InstallationError::ZeroStructuralReturnIdentity("continuation source block"),
            )?,
            target_block: BlockId::new(reader.u64()?).ok_or(
                InstallationError::ZeroStructuralReturnIdentity("continuation target block"),
            )?,
            cleanup: decode_unit_affine_cleanup(reader)?,
        });
    }
    Ok(continuations)
}
