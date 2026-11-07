//! Appending one producer's dense rows after another's.
//!
//! A table built from empty by one worker numbers its rows from the first
//! slot. Appending it after rows already in the owner's table moves every one
//! of its rows by exactly the owner's row count, so every handle the worker
//! minted into that table moves by the same amount and nothing else changes.
//! That holds only while both tables are dense and append-only: no freed
//! slot, no reused generation. [`shiftable`] says when it holds, and callers
//! refuse the append (and fall back to building serially) when it does not.

use arena::{Arena, Handle, HandleSpan};

/// The owner's row count when `from` can be appended to `into` by shifting:
/// both are dense and append-only, and both stamp new rows with the same
/// generation. `None` means a shifted handle could name the wrong row.
pub(crate) fn shiftable<T: Default>(into: &Arena<T>, from: &Arena<T>) -> Option<u32> {
    let (_, count) = into.append_epoch()?;
    from.append_epoch()?;
    (into.storage_view().fresh_generation == from.storage_view().fresh_generation)
        .then(|| u32::try_from(count).ok())
        .flatten()
}

/// `handle` moved past `rows` earlier rows. The invalid handle means "no row"
/// and stays invalid.
pub(crate) fn shift<T>(handle: Handle<T>, rows: u32) -> Handle<T> {
    if !handle.is_valid() {
        return handle;
    }
    Handle::from_parts(
        handle
            .arena_index()
            .checked_add(rows)
            .expect("appended rows exceed the handle range"),
        handle.generation(),
    )
}

/// `span` moved past `rows` earlier rows; an empty or invalid span stays as is.
pub(crate) fn shift_span<T>(span: HandleSpan<T>, rows: u32) -> HandleSpan<T> {
    if !span.start().is_valid() {
        return span;
    }
    HandleSpan::from_parts(shift(span.start(), rows), span.count())
}

/// Append every row of `from` to `into`, rewriting each row with `rewrite`.
pub(crate) fn append_rows<T: Default>(
    into: &mut Arena<T>,
    from: Arena<T>,
    mut rewrite: impl FnMut(T) -> T,
) {
    for row in from.into_items() {
        into.append(rewrite(row));
    }
}
