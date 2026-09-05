use target::Architecture;

use super::super::super::TextPlacementError;

pub(crate) fn validate(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), TextPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}
