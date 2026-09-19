//! The virtual filesystem the evaluator serves calls against: lookup,
//! directory listings, stat, open, read, write, seek, length and ownership.

use crate::interpreter::evaluator::{
    EvalResult, Evaluator, FIND_DATA_OUTPUT_BYTES, PreparedByteOutput, VIRTUAL_GID, VIRTUAL_UID,
    VirtualFd, checked_directory_name_snapshot_total, checked_directory_record_snapshot_total,
    host_open_flags, pack_dirent_records, portable_directory_entry_name,
};
use crate::{FilesystemMetadataObservation, FilesystemMetadataObservationKind};

fn immediate_virtual_child<'path>(path: &'path [u8], prefix: &[u8]) -> Option<&'path [u8]> {
    let rest = path.strip_prefix(prefix)?;
    if rest.is_empty() || rest.contains(&b'/') {
        None
    } else {
        Some(rest)
    }
}

impl<'program> Evaluator<'program> {
    /// Build the packed darwin `dirent` records for a directory: `.` and `..`
    /// then each IMMEDIATE child (files in `virtual_files`, subdirs in
    /// `virtual_dirs` directly under `dir_path/`). Each record is
    /// `[d_ino(8) d_seekoff(8) d_reclen@16(u16) d_namlen@18(u16) d_type@20(u8)
    /// d_name@21(namlen) NUL pad]`, `d_reclen = round_up_8(25 + namlen)` — the
    /// exact layout `___getdirentries64` produces, so byte counts and a parser
    /// agree with native.
    /// Resolve `name` RELATIVE to the open directory `dirfd` to a full virtual
    /// path (`dirfd`'s path + "/" + name). Returns None if `dirfd` is not an open
    /// directory descriptor. The `*at` ops do their path-joining here -- in Rust,
    /// the way the OS does natively -- so the Omega layer never builds a path.
    pub(crate) fn virtual_at_path(&self, dirfd: i32, name: &[u8]) -> Option<Vec<u8>> {
        let dir = self
            .virtual_fds
            .get(&dirfd)
            .filter(|descriptor| descriptor.is_dir)
            .map(|descriptor| descriptor.path.clone())?;
        let mut full = dir;
        full.push(b'/');
        full.extend_from_slice(name);
        Some(full)
    }

    /// The find-enumeration twin of `build_dirent_records` (fs rung 3a): the
    /// same entry set (".", "..", then the immediate children of `dir_path`)
    /// as (name, is_dir) pairs for a `find_first` cursor snapshot.
    pub(crate) fn build_find_entries(
        &self,
        dir_path: &[u8],
    ) -> EvalResult<std::collections::VecDeque<(Vec<u8>, bool)>> {
        let mut entries: std::collections::VecDeque<(Vec<u8>, bool)> =
            std::collections::VecDeque::from([(b".".to_vec(), true), (b"..".to_vec(), true)]);
        let mut name_bytes = checked_directory_name_snapshot_total(0, 1)?;
        name_bytes = checked_directory_name_snapshot_total(name_bytes, 2)?;
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_virtual_child(path, &prefix) {
                let name = portable_directory_entry_name(name);
                name_bytes = checked_directory_name_snapshot_total(name_bytes, name.len())?;
                entries.push_back((name.to_vec(), false));
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_virtual_child(path, &prefix) {
                let name = portable_directory_entry_name(name);
                name_bytes = checked_directory_name_snapshot_total(name_bytes, name.len())?;
                entries.push_back((name.to_vec(), true));
            }
        }
        Ok(entries)
    }

    /// Fill a caller find-data buffer (`&mut [u8]`, >= 320 bytes) the way
    /// `FindFirstFileA`/`FindNextFileA` write WIN32_FIND_DATAA: file
    /// attributes u32 little-endian at byte 0 (FILE_ATTRIBUTE_DIRECTORY 0x10 /
    /// FILE_ATTRIBUTE_NORMAL 0x80) and the NUL-terminated entry name at byte
    /// 44. Other fields are left zero.
    pub(crate) fn write_find_data(
        &self,
        output: &PreparedByteOutput,
        name: &[u8],
        is_dir: bool,
    ) -> EvalResult<()> {
        let mut record = vec![0u8; FIND_DATA_OUTPUT_BYTES];
        let attributes: u32 = if is_dir { 0x10 } else { 0x80 };
        record[0..4].copy_from_slice(&attributes.to_le_bytes());
        let name_len = name.len().min(259);
        record[44..44 + name_len].copy_from_slice(&name[..name_len]);
        output.write(&record)
    }

    pub(crate) fn build_dirent_records(&self, dir_path: &[u8]) -> EvalResult<Vec<u8>> {
        let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
        let mut record_bytes = checked_directory_record_snapshot_total(0, 1)?;
        record_bytes = checked_directory_record_snapshot_total(record_bytes, 2)?;
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_virtual_child(path, &prefix) {
                let name = portable_directory_entry_name(name);
                record_bytes = checked_directory_record_snapshot_total(record_bytes, name.len())?;
                entries.push((name.to_vec(), 8)); // DT_REG
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_virtual_child(path, &prefix) {
                let name = portable_directory_entry_name(name);
                record_bytes = checked_directory_record_snapshot_total(record_bytes, name.len())?;
                entries.push((name.to_vec(), 4)); // DT_DIR
            }
        }
        pack_dirent_records(&entries)
    }

    /// Fill a caller stat buffer from one target-neutral semantic metadata row
    /// using the exact selected target layout supplied by orchestration. The
    /// complete API carrier is zeroed first, including Linux-arm64's 16-byte
    /// tail beyond its 128-byte record, so prior mutable state cannot leak into
    /// either Omega decoding or package evidence.
    pub(crate) fn write_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        mode: u32,
        size: i64,
        mtime_secs: i64,
    ) -> EvalResult<()> {
        let observation = FilesystemMetadataObservation::new(1, kind, mode, size, mtime_secs);
        let carrier =
            self.canonical_metadata_carrier(observation, output.capacity(), "metadata output")?;
        output.write(&carrier)?;
        Ok(())
    }

    /// Mint a fresh descriptor over `path`; `create` truncates (or creates) the
    /// file first.
    pub(crate) fn virtual_open(&mut self, path: Vec<u8>, writable: bool, create: bool) -> i32 {
        if create {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: std::rc::Rc::new(std::cell::Cell::new(0)),
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// Write `bytes` at the descriptor's cursor (extending the file as needed),
    /// advancing the cursor. `None` if the fd is unknown or not writable.
    pub(crate) fn virtual_write(&mut self, fd: i32, bytes: &[u8]) -> Option<usize> {
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get();
        let content = self.virtual_files.get_mut(&path)?;
        let end = cursor + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[cursor..end].copy_from_slice(bytes);
        descriptor.cursor.set(end);
        Some(bytes.len())
    }

    /// Read up to `count` bytes from the descriptor's cursor, advancing it.
    /// `None` if the fd is unknown.
    pub(crate) fn virtual_read_n(&mut self, fd: i32, count: usize) -> Option<Vec<u8>> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(cursor..).unwrap_or(&[]);
        let take = available.len().min(count);
        let bytes = available[..take].to_vec();
        descriptor.cursor.set(cursor + take);
        Some(bytes)
    }

    /// Read up to `count` bytes starting at absolute `offset` WITHOUT moving the
    /// cursor (Rust `FileExt::read_at` / `pread`). `None` if the fd is unknown or
    /// the offset is negative. A read past end-of-file yields fewer (or zero) bytes.
    pub(crate) fn virtual_read_at(
        &mut self,
        fd: i32,
        offset: i64,
        count: usize,
    ) -> Option<Vec<u8>> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(offset as usize..).unwrap_or(&[]);
        let take = available.len().min(count);
        Some(available[..take].to_vec())
    }

    /// Write `bytes` at absolute `offset` (extending + zero-filling any gap) WITHOUT
    /// moving the cursor (Rust `FileExt::write_at` / `pwrite`). `None` if the fd is
    /// unknown, not writable, or the offset is negative.
    pub(crate) fn virtual_write_at(&mut self, fd: i32, offset: i64, bytes: &[u8]) -> Option<usize> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let start = offset as usize;
        let content = self.virtual_files.get_mut(&path)?;
        if bytes.is_empty() {
            return Some(0);
        }
        let end = start + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(bytes);
        Some(bytes.len())
    }

    /// Snapshot the complete file state modeled by the virtual provider under
    /// a second name. Hard-link aliasing after this operation is intentionally
    /// outside the model; this is exact only while neither name is mutated.
    pub(crate) fn virtual_hard_link_file(&mut self, existing: &[u8], link: Vec<u8>) -> bool {
        let Some(content) = self.virtual_files.get(existing).cloned() else {
            return false;
        };
        let permissions = self.virtual_perms.get(existing).copied();
        let modification_time = self.virtual_times.get(existing).copied();

        self.virtual_files.insert(link.clone(), content);
        match permissions {
            Some(mode) => {
                self.virtual_perms.insert(link.clone(), mode);
            }
            None => {
                self.virtual_perms.remove(&link);
            }
        }
        match modification_time {
            Some(time) => {
                self.virtual_times.insert(link, time);
            }
            None => {
                self.virtual_times.remove(&link);
            }
        }
        true
    }

    /// `open(path, flags)`: model the O_CREAT/O_TRUNC/O_APPEND/access bits.
    /// Returns a fresh fd, or -1 if the path is absent and O_CREAT is not set.
    pub(crate) fn virtual_open_flags(&mut self, path: Vec<u8>, flags: i32) -> i32 {
        // Follow one symlink level (the canonicalize/read_link model): native
        // open on BOTH families resolves symlinks, and the hermetic open never
        // did -- surfaced when the windows canonicalize composition made open
        // its entry point. The descriptor stores the RESOLVED path, so
        // handle-keyed consumers (final_path_name_by_handle) report the final
        // target exactly like Win32.
        let path = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
        let exists = self.virtual_files.contains_key(&path);
        let o_creat = host_open_flags::o_creat(flags);
        let o_trunc = host_open_flags::o_trunc(flags);
        let o_append = host_open_flags::o_append(flags);
        let writable = flags & 0x3 != 0; // O_WRONLY | O_RDWR (universal)
        // Opening a directory for writing is EISDIR (Rust `ErrorKind::IsADirectory`).
        // Checked before the ENOENT test so a dir path (never in `virtual_files`)
        // reports the more specific kind.
        if self.virtual_dirs.contains(&path) && writable {
            self.virtual_errno = 21; // EISDIR
            return -1;
        }
        // Permission enforcement: opening a chmod'd path fails with EACCES when
        // the needed bit is clear — the owner-write bit (0o200) for a write-open,
        // or the owner-read bit (0o400) for a read-open (Rust
        // `ErrorKind::PermissionDenied`).
        let needed_bit = if writable { 0o200 } else { 0o400 };
        if self
            .virtual_perms
            .get(&path)
            .is_some_and(|mode| mode & needed_bit == 0)
        {
            self.virtual_errno = 13; // EACCES
            return -1;
        }
        // Read-open of a DIRECTORY: POSIX allows opening a dir read-only (the
        // basis for `read_dir`). Mint a dir descriptor. Checked before the ENOENT
        // test since a dir path is never in `virtual_files`. (This also aligns
        // `exists`/`try_exists` on a dir with native, where opening a dir works.)
        if !writable && self.virtual_dirs.contains(&path) {
            let fd = self.virtual_next_fd;
            self.virtual_next_fd += 1;
            self.virtual_fds.insert(
                fd,
                VirtualFd {
                    path,
                    cursor: std::rc::Rc::new(std::cell::Cell::new(0)),
                    writable: false,
                    is_dir: true,
                },
            );
            return fd;
        }
        if !exists && !o_creat {
            self.virtual_errno = 2; // ENOENT
            return -1;
        }
        if !exists || o_trunc {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let cursor = if o_append {
            self.virtual_files.get(&path).map_or(0, Vec::len)
        } else {
            0
        };
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: std::rc::Rc::new(std::cell::Cell::new(cursor)),
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// `lseek(fd, offset, whence)`: reposition the cursor, returning the new
    /// absolute offset. `None` on unknown fd, bad whence, or a negative result.
    pub(crate) fn virtual_seek(&mut self, fd: i32, offset: i64, whence: i32) -> Option<i64> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get() as i64;
        let len = self.virtual_files.get(&path).map_or(0, Vec::len) as i64;
        let new_pos = match whence {
            0 => offset,          // SEEK_SET
            1 => cursor + offset, // SEEK_CUR
            2 => len + offset,    // SEEK_END
            _ => return None,
        };
        if new_pos < 0 {
            return None;
        }
        descriptor.cursor.set(new_pos as usize);
        Some(new_pos)
    }

    /// `ftruncate(fd, length)`: resize the file backing `fd` (truncate or
    /// zero-extend). Returns 0 on success, -1 on an unknown fd/path.
    pub(crate) fn virtual_set_len(&mut self, fd: i32, length: i64) -> i64 {
        let Some(descriptor) = self.virtual_fds.get(&fd) else {
            return -1;
        };
        let path = descriptor.path.clone();
        let Some(content) = self.virtual_files.get_mut(&path) else {
            return -1;
        };
        content.resize(length.max(0) as usize, 0);
        0
    }

    /// The non-root `chown`/`fchown`/`lchown` rule shared by the ownership
    /// handlers: a change to the CURRENT owner -- or a uid/gid of -1, meaning
    /// "leave that component unchanged" -- is a permitted no-op (returns 0); any
    /// OTHER owner is EPERM (sets errno 1, returns -1). Mirrors what the native
    /// syscalls do for a normal (non-root) user, keeping the two engines'
    /// differential consistent.
    pub(crate) fn virtual_chown_result(&mut self, uid: i32, gid: i32) -> i64 {
        let effective_uid = if uid == -1 { VIRTUAL_UID as i32 } else { uid };
        let effective_gid = if gid == -1 { VIRTUAL_GID as i32 } else { gid };
        if effective_uid == VIRTUAL_UID as i32 && effective_gid == VIRTUAL_GID as i32 {
            0
        } else {
            self.virtual_errno = 1; // EPERM
            -1
        }
    }
}
