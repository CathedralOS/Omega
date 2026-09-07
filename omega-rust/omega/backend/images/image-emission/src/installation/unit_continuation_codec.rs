//! Ordered continuation edges retain their own zero-byte cleanup records.

use super::function_affine_cleanup_codec::{
    decode_unit_affine_cleanup, encode_unit_affine_cleanup,
};
use super::unit_scalar_codec::{decode_scalar_type, encode_scalar_type};
use super::{InstallationError, Reader, push_u32, push_u64};
use machine_code::UnitContinuationRecord;
use semantic_vocabulary::{BlockId, ValueId};

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
        push_u32(
            bytes,
            u32::try_from(continuation.bindings.len())
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
        );
        for binding in &continuation.bindings {
            push_u64(bytes, binding.parameter.get());
            push_u64(bytes, binding.argument.get());
            encode_scalar_type(bytes, binding.scalar_type)?;
        }
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
            bindings: decode_bindings(reader)?,
            cleanup: decode_unit_affine_cleanup(reader)?,
        });
    }
    Ok(continuations)
}

fn decode_bindings(
    reader: &mut Reader<'_>,
) -> Result<Vec<target_operations::ValueBinding>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
    if count > reader.remaining() / 20 {
        return Err(InstallationError::UnexpectedEnd);
    }
    (0..count)
        .map(|_| {
            Ok(target_operations::ValueBinding {
                parameter: ValueId::new(reader.u64()?)
                    .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
                argument: ValueId::new(reader.u64()?)
                    .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
                scalar_type: decode_scalar_type(reader)?,
            })
        })
        .collect()
}
