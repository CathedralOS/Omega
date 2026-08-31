use super::super::RelocationFreeTextSectionPlacementError;

pub(crate) fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    u64::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

pub(super) fn u64_to_usize(value: u64) -> Result<usize, RelocationFreeTextSectionPlacementError> {
    usize::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}
