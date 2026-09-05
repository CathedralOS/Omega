use super::TextPlacementError;

pub(super) fn usize_to_u64(value: usize) -> Result<u64, TextPlacementError> {
    u64::try_from(value).map_err(|_| TextPlacementError::OffsetOverflow)
}

pub(super) fn u64_to_usize(value: u64) -> Result<usize, TextPlacementError> {
    usize::try_from(value).map_err(|_| TextPlacementError::OffsetOverflow)
}
