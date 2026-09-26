//! Directory entries as the portable std contract reads them: names bounded
//! to the entry limit, snapshot totals checked against the directory
//! snapshot ceiling, and `(name, d_type)` pairs packed as darwin `dirent`
//! records, shared by the virtual and real filesystems.

use super::{EvalResult, Halt, MAX_DIRECTORY_ENTRY_NAME_BYTES, MAX_DIRECTORY_SNAPSHOT_BYTES};

/// Pack `(name, d_type)` entries as darwin `dirent` records, the layout native
/// `___getdirentries64` returns (reclen u16 @16, namlen u16 @18, d_type u8
/// @20, name @21, records 8-byte aligned) -- so a parser is identical on both
/// engines. Shared by the virtual fs (`build_dirent_records`) and the real-fs
/// provider (`try_real_filesystem_call`'s `read_dir`), which differ only in
/// where the names come from.
pub(super) fn portable_directory_entry_name(name: &[u8]) -> &[u8] {
    &name[..name.len().min(MAX_DIRECTORY_ENTRY_NAME_BYTES)]
}

pub(super) fn dirent_record_extent(name_len: usize) -> EvalResult<usize> {
    if name_len > MAX_DIRECTORY_ENTRY_NAME_BYTES {
        return Err(Halt::Trap(format!(
            "untruncated directory entry name reached the dirent packer ({name_len} > {MAX_DIRECTORY_ENTRY_NAME_BYTES})"
        )));
    }
    let unaligned = 25usize.checked_add(name_len).ok_or_else(|| {
        Halt::Resource("directory entry record extent overflowed usize".to_owned())
    })?;
    let reclen = unaligned
        .checked_add(7)
        .map(|extent| extent / 8 * 8)
        .ok_or_else(|| {
            Halt::Resource("directory entry record alignment overflowed usize".to_owned())
        })?;
    debug_assert!(reclen <= u16::MAX as usize);
    Ok(reclen)
}

pub(super) fn checked_directory_name_snapshot_total(
    current: usize,
    name_len: usize,
) -> EvalResult<usize> {
    if name_len > MAX_DIRECTORY_ENTRY_NAME_BYTES {
        return Err(Halt::Trap(format!(
            "untruncated directory entry name reached snapshot accounting ({name_len} > {MAX_DIRECTORY_ENTRY_NAME_BYTES})"
        )));
    }
    let total = current.checked_add(name_len).ok_or_else(|| {
        Halt::Resource("directory enumeration retained-name extent overflowed usize".to_owned())
    })?;
    if total > MAX_DIRECTORY_SNAPSHOT_BYTES {
        return Err(Halt::Resource(format!(
            "directory enumeration retained names exceeded their {MAX_DIRECTORY_SNAPSHOT_BYTES}-byte logical payload ceiling"
        )));
    }
    Ok(total)
}

pub(super) fn checked_directory_record_snapshot_total(
    current: usize,
    name_len: usize,
) -> EvalResult<usize> {
    let reclen = dirent_record_extent(name_len)?;
    let total = current.checked_add(reclen).ok_or_else(|| {
        Halt::Resource("directory enumeration packed-record extent overflowed usize".to_owned())
    })?;
    if total > MAX_DIRECTORY_SNAPSHOT_BYTES {
        return Err(Halt::Resource(format!(
            "directory enumeration packed records exceeded their {MAX_DIRECTORY_SNAPSHOT_BYTES}-byte logical payload ceiling"
        )));
    }
    Ok(total)
}

pub(super) fn pack_dirent_records(entries: &[(Vec<u8>, u8)]) -> EvalResult<Vec<u8>> {
    let total = entries.iter().try_fold(0usize, |total, (name, _)| {
        checked_directory_record_snapshot_total(total, portable_directory_entry_name(name).len())
    })?;
    let mut buffer = Vec::with_capacity(total);
    for (name, d_type) in entries {
        let name = portable_directory_entry_name(name);
        let namlen = name.len();
        let reclen = dirent_record_extent(namlen)?;
        let start = buffer.len();
        buffer.resize(start + reclen, 0);
        buffer[start + 16..start + 18].copy_from_slice(&(reclen as u16).to_le_bytes());
        buffer[start + 18..start + 20].copy_from_slice(&(namlen as u16).to_le_bytes());
        buffer[start + 20] = *d_type;
        buffer[start + 21..start + 21 + namlen].copy_from_slice(name);
    }
    debug_assert_eq!(buffer.len(), total);
    Ok(buffer)
}

/// Select the next complete-record window from a packed Darwin dirent stream.
/// `getdirentries64` never splits a record across caller buffers, so the
/// interpreter advances its synthetic byte cursor only through the last record
/// that fits in `count` bytes. The std wrapper uses a 512-byte buffer, larger
/// than the maximum packed record produced above.
pub(super) fn dirent_record_chunk(records: &[u8], start: usize, count: usize) -> (&[u8], usize) {
    if start >= records.len() || count == 0 {
        return (&records[0..0], start);
    }

    let limit = start.saturating_add(count).min(records.len());
    let mut end = start;
    while end + 18 <= records.len() {
        let reclen = u16::from_le_bytes([records[end + 16], records[end + 17]]) as usize;
        if reclen == 0 || end + reclen > records.len() || end + reclen > limit {
            break;
        }
        end += reclen;
    }
    (&records[start..end], end)
}
