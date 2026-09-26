//! Directory enumeration: `readdir` windows and the Win32 find cursor.

use crate::interpreter::evaluator::{EvalResult, PreparedFilesystemCall, dirent_record_chunk};

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(super) fn serve_read_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadDir {
            fd,
            buffer,
            count,
            position,
        } = call
        else {
            unreachable!("dispatched serve_read_dir")
        };
        Ok({
            // `read_dir(fd, buf, count, &position)`: pack the directory's
            // entries as Darwin `dirent` records and return the next window
            // of complete records. `position` is a synthetic byte cursor, so
            // repeated calls drain directories larger than one buffer just
            // like native `___getdirentries64`.
            let dir_path = self
                .virtual_fds
                .get(&fd)
                .filter(|descriptor| descriptor.is_dir)
                .map(|descriptor| descriptor.path.clone());
            match dir_path {
                None => {
                    // Unknown fd -> EBADF; a live non-dir fd -> ENOTDIR.
                    self.virtual_errno = if self.virtual_fds.contains_key(&fd) {
                        20 // ENOTDIR
                    } else {
                        9 // EBADF
                    };
                    -1
                }
                Some(path) => {
                    let records = self.build_dirent_records(&path)?;
                    let start = position.initial.max(0) as usize;
                    let (chunk, next_position) = dirent_record_chunk(&records, start, count.host);
                    if chunk.is_empty() {
                        0
                    } else {
                        let n = chunk.len();
                        buffer.write(chunk)?;
                        position.write(next_position as i64)?;
                        n as i64
                    }
                }
            }
        })
    }

    pub(super) fn serve_find_first(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindFirst { pattern, data } = call else {
            unreachable!("dispatched serve_find_first")
        };
        Ok({
            // `find_first(pattern, &data)` -- the windows dir-walk seam (fs
            // rung 3a). `pattern` is `dir/*`: the impl joins with `/`, which
            // Win32 accepts natively and which matches the hermetic FS keys
            // byte-exactly. Snapshot the directory's entries (".", "..",
            // then the immediate children -- the same set read_dir packs)
            // into a cursor keyed by a fresh handle, fill the FIRST entry's
            // find-data record, and return the handle; -1
            // (INVALID_HANDLE_VALUE, ENOENT) when the directory does not
            // exist. A real directory always yields "." first, so an open
            // enumeration always has a first entry -- exactly Win32.
            let entries = pattern
                .strip_suffix(b"/*")
                .filter(|dir_path| self.virtual_dirs.contains(*dir_path))
                .map(|dir_path| self.build_find_entries(dir_path))
                .transpose()?;
            match entries {
                Some(mut entries) => {
                    let (name, is_dir) =
                        entries.pop_front().expect("dot entries are always present");
                    self.write_find_data(&data, &name, is_dir)?;
                    let handle = self.virtual_next_find;
                    self.virtual_next_find += 1;
                    self.virtual_finds.insert(handle, entries);
                    handle
                }
                None => {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
        })
    }

    pub(super) fn serve_find_next(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindNext { handle, data } = call else {
            unreachable!("dispatched serve_find_next")
        };
        Ok({
            // `find_next(handle, &data)`: fill the next snapshotted entry
            // (1 = filled, 0 = end-of-enumeration or unknown handle).
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

    pub(super) fn serve_find_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::FindClose { handle } = call else {
            unreachable!("dispatched serve_find_close")
        };
        Ok({
            // `find_close(handle)`: release the cursor (BOOL, like Win32).
            if self.virtual_finds.remove(&handle).is_some() {
                1
            } else {
                0
            }
        })
    }
}
