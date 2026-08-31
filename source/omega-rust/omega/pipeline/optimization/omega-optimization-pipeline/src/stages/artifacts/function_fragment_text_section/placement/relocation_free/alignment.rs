use omega_target::Architecture;

use super::super::super::RelocationFreeTextSectionPlacementError;

pub(super) fn validate(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}
