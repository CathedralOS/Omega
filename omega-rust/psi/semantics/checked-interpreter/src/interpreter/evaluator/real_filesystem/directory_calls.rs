//! Directory enumeration: `readdir` windows and the Win32 find cursor.

use super::super::{EvalResult, PreparedFilesystemCall};
use super::{DirectoryEntrySnapshotKind, EBADF, ENOENT, real_directory_entries};

impl<'program> super::super::Evaluator<'program> {
    pub(super) fn real_read_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadDir {
            fd,
            buffer,
            count,
            position,
        } = call
        else {
            unreachable!("dispatched real_read_dir")
        };
        Ok({
            // `read_dir(fd, buf, count, &position)` -- the virtual
            // dispatcher's contract, mirrored. Pack `.`/`..` plus immediate
            // children as Darwin dirent records and return the next window
            // of complete records. The synthetic byte cursor lets repeated
            // calls drain directories larger than one caller buffer. Names
            // come from `std::fs::read_dir` and are sorted for determinism;
            // native getdirentries order remains filesystem-defined.
            let listed = {
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_directory_entries(
                        &entry.path,
                        DirectoryEntrySnapshotKind::PackedRecords,
                    ),
                    None => Ok(Err(EBADF)),
                }
            }?;
            match listed {
                Ok(entries) => {
                    let records = super::super::pack_dirent_records(&entries)?;
                    let start = position.initial.max(0) as usize;
                    let (chunk, next_position) =
                        super::super::dirent_record_chunk(&records, start, count.host);
                    if chunk.is_empty() {
                        0
                    } else {
                        let n = chunk.len();
                        buffer.write(chunk)?;
                        position.write(next_position as i64)?;
                        n as i64
                    }
                }
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    -1
                }
            }
        })
    }

    pub(super) fn real_find_first(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindFirst { pattern, data } = call else {
            unreachable!("dispatched real_find_first")
        };
        Ok({
            // `find_first(pattern, &data)` -- the windows dir-walk seam
            // (fs rung 3a) served against the real filesystem: strip the
            // `/*` tail (the impl joins with `/`, which Win32 accepts),
            // list the directory (the same dot-prefixed sorted set
            // read_dir packs), snapshot the tail into a cursor keyed by a
            // fresh handle, and fill the FIRST entry's find-data record.
            let listed = match pattern.strip_suffix(b"/*") {
                Some(dir_path) => match self.authorized_path(dir_path, false, 0) {
                    Some(path) => {
                        real_directory_entries(&path, DirectoryEntrySnapshotKind::FindCursor)
                    }
                    None => {
                        return Ok(-1);
                    }
                },
                None => Ok(Err(ENOENT)),
            }?;
            match listed {
                Ok(entries) => {
                    let mut queue: std::collections::VecDeque<(Vec<u8>, bool)> = entries
                        .into_iter()
                        .map(|(name, d_type)| (name, d_type == 4))
                        .collect();
                    let (name, is_dir) = queue.pop_front().expect("dot entries are always present");
                    self.write_find_data(&data, &name, is_dir)?;
                    let handle = self.virtual_next_find;
                    self.virtual_next_find += 1;
                    self.virtual_finds.insert(handle, queue);
                    handle
                }
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    -1
                }
            }
        })
    }

    pub(super) fn real_find_next(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindNext { handle, data } = call else {
            unreachable!("dispatched real_find_next")
        };
        Ok({
            // Cursor-only (the snapshot was taken at find_first) -- the
            // same arm shape as the hermetic dispatcher.
            match self
                .virtual_finds
                .get_mut(&handle)
                .and_then(std::collections::VecDeque::pop_front)
            {
                Some((name, is_dir)) => {
                    self.write_find_data(&data, &name, is_dir)?;
                    1
                }
                None => 0,
            }
        })
    }

    pub(super) fn real_find_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindClose { handle } = call else {
            unreachable!("dispatched real_find_close")
        };
        Ok({
            if self.virtual_finds.remove(&handle).is_some() {
                1
            } else {
                0
            }
        })
    }
}
