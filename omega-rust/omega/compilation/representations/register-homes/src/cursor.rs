//! Shared bounded byte cursor for the crate's wire decoders: a slice view
//! plus a position. Truncation is the only failure the cursor itself owns;
//! each decoder maps it into its own error enum through `From<Truncated>`
//! and the u64-sized length read through [`TakeError`].

pub(crate) use optimization_core::cursor::{Cursor, TakeError, Truncated};

/// Presence-flagged optional decode: byte 0 yields None, byte 1 runs the
/// payload reader, any other tag is the caller's unknown-tag error.
pub(crate) fn optional<T, E>(
    cursor: &mut Cursor<'_>,
    read: impl FnOnce(&mut Cursor<'_>) -> Result<T, E>,
    unknown_tag: impl FnOnce(u8) -> E,
) -> Result<Option<T>, E>
where
    E: From<Truncated>,
{
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(read(cursor)?)),
        tag => Err(unknown_tag(tag)),
    }
}
