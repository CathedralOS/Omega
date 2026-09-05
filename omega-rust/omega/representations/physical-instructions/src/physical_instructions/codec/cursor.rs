//! Cursor primitives and pre-allocation field-error translation.

use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;

use super::PostAllocationMachineDecodeError;

pub(super) fn map_field_error(
    error: selected_instructions::PreAllocationMachineEffectDecodeError,
) -> PostAllocationMachineDecodeError {
    match error {
        selected_instructions::PreAllocationMachineEffectDecodeError::Truncated => {
            PostAllocationMachineDecodeError::Truncated
        }
        _ => PostAllocationMachineDecodeError::InvalidField,
    }
}

pub(super) fn take<'a>(
    cursor: &mut effect_codec::Cursor<'a>,
    count: usize,
) -> Result<&'a [u8], PostAllocationMachineDecodeError> {
    cursor.take(count).map_err(map_field_error)
}

pub(super) fn array<const N: usize>(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<[u8; N], PostAllocationMachineDecodeError> {
    cursor.array().map_err(map_field_error)
}

pub(super) fn byte(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u8, PostAllocationMachineDecodeError> {
    cursor.byte().map_err(map_field_error)
}

pub(super) fn u16_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u16, PostAllocationMachineDecodeError> {
    cursor.u16().map_err(map_field_error)
}

pub(super) fn u32_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u32, PostAllocationMachineDecodeError> {
    cursor.u32().map_err(map_field_error)
}

pub(super) fn u64_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u64, PostAllocationMachineDecodeError> {
    cursor.u64().map_err(map_field_error)
}

pub(super) fn length(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<usize, PostAllocationMachineDecodeError> {
    cursor.length().map_err(map_field_error)
}

pub(super) fn decode_units(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<Vec<register_model::RegisterUnitId>, PostAllocationMachineDecodeError> {
    effect_codec::decode_units(cursor).map_err(map_field_error)
}
