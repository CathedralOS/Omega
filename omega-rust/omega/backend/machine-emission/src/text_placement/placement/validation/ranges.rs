//! Checked byte ranges and preservation outside explicitly patched fields.
use super::TextPlacementError;

pub(super) fn unchanged_bytes(
    source: &[u8],
    candidate: &[u8],
    mut patches: Vec<(u64, u64)>,
) -> Result<(), TextPlacementError> {
    if source.len() != candidate.len() {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    patches.sort_unstable();
    let mut cursor = 0;
    for (start, end) in patches {
        if start < cursor || end < start || end > source.len() as u64 {
            return Err(TextPlacementError::ArtifactMismatch);
        }
        if bytes(source, cursor, start - cursor)? != bytes(candidate, cursor, start - cursor)? {
            return Err(TextPlacementError::ArtifactMismatch);
        }
        cursor = end;
    }
    if bytes(source, cursor, source.len() as u64 - cursor)?
        != bytes(candidate, cursor, candidate.len() as u64 - cursor)?
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

pub(super) fn bytes(source: &[u8], start: u64, count: u64) -> Result<&[u8], TextPlacementError> {
    let end = add(start, count)?;
    let start = usize::try_from(start).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let end = usize::try_from(end).map_err(|_| TextPlacementError::OffsetOverflow)?;
    source
        .get(start..end)
        .ok_or(TextPlacementError::ArtifactMismatch)
}
pub(super) fn add(left: u64, right: u64) -> Result<u64, TextPlacementError> {
    left.checked_add(right)
        .ok_or(TextPlacementError::OffsetOverflow)
}
