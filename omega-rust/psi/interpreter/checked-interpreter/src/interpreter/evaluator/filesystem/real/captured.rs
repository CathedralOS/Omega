//! Captured input operations over retained source storage. A logical root has
//! no host spelling. Physical output descriptors keep their existing sponsor
//! protocol, but both stores mint descriptors through RealFs's one allocator.

use super::{EACCES, EBADF, EINVAL, ENOENT, ENOTDIR, RealFs, real_fs};
use crate::FilesystemGrantRootIdentity;
use crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemPath;
use crate::interpreter::evaluator::{EvalResult, Halt, PreparedFilesystemCall, Value};
use source::{SourceSnapshot, SourceSnapshotEntry, SourceSnapshotError};

pub(super) struct CapturedDescriptor {
    pub(super) root: FilesystemGrantRootIdentity,
    /// Retired descriptors retain only their provider/root coordinate. They
    /// must return EBADF through the captured dispatcher, never fall through
    /// to a physical table. Closing drops path storage and cursor custody.
    open: Option<CapturedOpenDescriptor>,
}

struct CapturedOpenDescriptor {
    relative: Vec<u8>,
    /// Duplicates share an open description, not a copied cursor. This is
    /// evaluator-owned state; immutable inventory storage needs no interior
    /// mutability or physical file handles.
    description: i32,
}

pub(super) struct CapturedOpenDescription {
    cursor: u64,
    references: usize,
}

fn snapshot_errno(error: SourceSnapshotError) -> i32 {
    match error {
        SourceSnapshotError::NotFound => ENOENT,
        SourceSnapshotError::NotDirectory => ENOTDIR,
        SourceSnapshotError::NotFile => 21,
        SourceSnapshotError::InvalidPath
        | SourceSnapshotError::NotSymlink
        | SourceSnapshotError::InvalidRange => EINVAL,
    }
}

/// The cursor is an authored input, not permission to enter a record halfway.
/// Validate it against record boundaries in the freshly encoded logical list.
fn captured_directory_position_is_valid(records: &[u8], requested: usize) -> bool {
    let mut position = 0usize;
    while position < requested {
        let Some(length_bytes) = records.get(position + 16..position + 18) else {
            return false;
        };
        let length = usize::from(u16::from_le_bytes([length_bytes[0], length_bytes[1]]));
        if length == 0 {
            return false;
        }
        let Some(next) = position
            .checked_add(length)
            .filter(|next| *next <= records.len())
        else {
            return false;
        };
        position = next;
    }
    position == requested && requested <= records.len()
}

impl RealFs {
    fn captured_snapshot(&self, root: FilesystemGrantRootIdentity) -> Result<&SourceSnapshot, i32> {
        self.captured_roots.get(&root).ok_or(EACCES)
    }

    fn open_captured(
        &mut self,
        root: FilesystemGrantRootIdentity,
        relative: Vec<u8>,
        flags: i32,
    ) -> Result<i64, i32> {
        use crate::interpreter::evaluator::filesystem::host_open_flags;
        if flags & 3 != 0
            || host_open_flags::o_creat(flags)
            || host_open_flags::o_trunc(flags)
            || host_open_flags::o_append(flags)
        {
            return Err(EACCES);
        }
        if flags != 0 {
            return Err(45); // native open flags are not source snapshot vocabulary
        }
        match self
            .captured_snapshot(root)?
            .lookup(&relative)
            .map_err(snapshot_errno)?
        {
            SourceSnapshotEntry::File(_) => {}
            SourceSnapshotEntry::Directory => return Err(21),
            SourceSnapshotEntry::Symlink(_) => return Err(40), // inert, never followed
        }
        let descriptor = self.allocate_descriptor()?;
        self.captured_descriptions.insert(
            descriptor,
            CapturedOpenDescription {
                cursor: 0,
                references: 1,
            },
        );
        self.captured_descriptors.insert(
            descriptor,
            CapturedDescriptor {
                root,
                open: Some(CapturedOpenDescriptor {
                    relative,
                    description: descriptor,
                }),
            },
        );
        Ok(i64::from(descriptor))
    }

    fn close_captured(&mut self, descriptor: i32) -> Result<i64, i32> {
        let entry = self
            .captured_descriptors
            .get_mut(&descriptor)
            .ok_or(EBADF)?;
        let open = entry.open.take().ok_or(EBADF)?;
        let description = self
            .captured_descriptions
            .get_mut(&open.description)
            .ok_or(EBADF)?;
        description.references -= 1;
        if description.references == 0 {
            self.captured_descriptions.remove(&open.description);
        }
        Ok(0)
    }

    fn duplicate_captured(&mut self, descriptor: i32) -> Result<i64, i32> {
        let entry = self.captured_descriptors.get(&descriptor).ok_or(EBADF)?;
        let open = entry.open.as_ref().ok_or(EBADF)?;
        let description_identity = open.description;
        let duplicate = CapturedDescriptor {
            root: entry.root,
            open: Some(CapturedOpenDescriptor {
                relative: open.relative.clone(),
                description: description_identity,
            }),
        };
        let descriptor = self.allocate_descriptor()?;
        let description = self
            .captured_descriptions
            .get_mut(&description_identity)
            .ok_or(EBADF)?;
        description.references = description.references.checked_add(1).ok_or(24)?;
        self.captured_descriptors.insert(descriptor, duplicate);
        Ok(i64::from(descriptor))
    }

    fn seek_captured(&mut self, descriptor: i32, offset: i64, whence: i32) -> Result<i64, i32> {
        let entry = self.captured_descriptors.get(&descriptor).ok_or(EBADF)?;
        let open = entry.open.as_ref().ok_or(EBADF)?;
        let length = match self
            .captured_snapshot(entry.root)?
            .lookup(&open.relative)
            .map_err(snapshot_errno)?
        {
            SourceSnapshotEntry::File(bytes) => bytes.len() as u64,
            _ => return Err(EINVAL),
        };
        let description = self
            .captured_descriptions
            .get_mut(&open.description)
            .ok_or(EBADF)?;
        let origin = match whence {
            0 => 0,
            1 => description.cursor,
            2 => length,
            _ => return Err(EINVAL),
        };
        let position = origin
            .checked_add_signed(offset)
            .filter(|position| *position <= length)
            .ok_or(EINVAL)?;
        description.cursor = position;
        i64::try_from(position).map_err(|_| EINVAL)
    }
}

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    fn captured_path(
        &mut self,
        path: PreparedFilesystemPath,
    ) -> Result<(FilesystemGrantRootIdentity, Vec<u8>), i32> {
        let PreparedFilesystemPath::Captured { root, relative } = path else {
            return Err(EACCES);
        };
        if !self.record_authorized_path(0, false, root, relative.clone()) {
            return Err(EACCES);
        }
        Ok((root, relative))
    }

    pub(in crate::interpreter::evaluator) fn serve_captured_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
        plan: &crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<Value> {
        if self.real_fs.as_ref().is_some_and(|filesystem| {
            plan.inputs.iter().any(|input| {
                i32::try_from(input.raw)
                    .ok()
                    .and_then(|descriptor| filesystem.captured_descriptors.get(&descriptor))
                    .is_some_and(|entry| entry.open.is_none())
            })
        }) {
            self.real_fs_mut().errno = EBADF;
            return Ok(Value::Int(-1));
        }
        // This match is closed, including refusals. Native-only metadata is
        // not fabricated from a captured kind/length; callers requesting host
        // stat, locks or host handles receive an unsupported-operation result.
        let outcome: Result<i64, i32> = match call {
            PreparedFilesystemCall::Open { path, flags }
            | PreparedFilesystemCall::OpenCreate { path, flags, .. } => {
                self.captured_path(path).and_then(|(root, relative)| {
                    self.real_fs_mut().open_captured(root, relative, flags)
                })
            }
            PreparedFilesystemCall::Read { fd, buffer, count } => {
                self.read_captured(fd, &buffer, count.host as u64, None)?
            }
            PreparedFilesystemCall::ReadAt {
                fd,
                buffer,
                count,
                offset,
            } => match u64::try_from(offset) {
                Ok(offset) => self.read_captured(fd, &buffer, count.host as u64, Some(offset))?,
                Err(_) => Err(EINVAL),
            },
            PreparedFilesystemCall::Close { fd } => self.real_fs_mut().close_captured(fd),
            PreparedFilesystemCall::Duplicate { fd } => self.real_fs_mut().duplicate_captured(fd),
            PreparedFilesystemCall::Seek { fd, offset, whence } => {
                self.real_fs_mut().seek_captured(fd, offset, whence)
            }
            PreparedFilesystemCall::ReadLink {
                path,
                buffer,
                count,
            } => match self.captured_path(path) {
                Ok((root, relative)) => {
                    let real = real_fs(self)
                        .ok_or_else(|| Halt::Trap("captured provider disappeared".into()))?;
                    match real
                        .captured_snapshot(root)
                        .and_then(|snapshot| snapshot.read_link(&relative).map_err(snapshot_errno))
                    {
                        Ok(bytes) => {
                            let bytes = &bytes[..bytes.len().min(count.host)];
                            buffer.write(bytes)?;
                            Ok(bytes.len() as i64)
                        }
                        Err(errno) => Err(errno),
                    }
                }
                Err(errno) => Err(errno),
            },
            PreparedFilesystemCall::OpenAt { dirfd, name, flags } => {
                let coordinate = self.real_fs.as_ref().and_then(|real| {
                    let entry = real.captured_descriptors.get(&dirfd)?;
                    let open = entry.open.as_ref()?;
                    let snapshot = real.captured_snapshot(entry.root).ok()?;
                    if snapshot.lookup(&open.relative).ok()? != SourceSnapshotEntry::Directory {
                        return None;
                    }
                    let mut relative = open.relative.clone();
                    if !relative.is_empty() {
                        relative.push(b'/');
                    }
                    relative.extend(name);
                    Some((entry.root, relative))
                });
                match coordinate {
                    Some((root, relative))
                        if self.record_authorized_path(1, false, root, relative.clone()) =>
                    {
                        self.real_fs_mut().open_captured(root, relative, flags)
                    }
                    _ => Err(ENOTDIR),
                }
            }
            PreparedFilesystemCall::ReadDir {
                fd,
                buffer,
                count,
                position,
            } => {
                let real = real_fs(self)
                    .ok_or_else(|| Halt::Trap("captured provider disappeared".into()))?;
                let listed = real
                    .captured_descriptors
                    .get(&fd)
                    .ok_or(EBADF)
                    .and_then(|entry| {
                        let open = entry.open.as_ref().ok_or(EBADF)?;
                        real.captured_snapshot(entry.root)?
                            .list(&open.relative)
                            .map_err(snapshot_errno)
                    });
                match listed {
                    Ok(children) => {
                        // Names are exact. The native dirent carrier has a
                        // smaller name extent than the source protocol; refuse
                        // that carrier rather than truncate or alias names.
                        if children.iter().any(|(name, _)| {
                            name.len()
                                > crate::interpreter::evaluator::MAX_DIRECTORY_ENTRY_NAME_BYTES
                        }) {
                            return Err(Halt::Unsupported("captured directory name exceeds native listing carrier; use source snapshot list".into()));
                        }
                        let entries: Vec<_> = children
                            .into_iter()
                            .map(|(name, kind)| {
                                (
                                    name.to_vec(),
                                    match kind {
                                        SourceSnapshotEntry::Directory => 4,
                                        SourceSnapshotEntry::File(_) => 8,
                                        SourceSnapshotEntry::Symlink(_) => 10,
                                    },
                                )
                            })
                            .collect();
                        let records = crate::interpreter::evaluator::pack_dirent_records(&entries)?;
                        match usize::try_from(position.initial) {
                            Ok(start) if captured_directory_position_is_valid(&records, start) => {
                                let (chunk, next) =
                                    crate::interpreter::evaluator::dirent_record_chunk(
                                        &records, start, count.host,
                                    );
                                buffer.write(chunk)?;
                                position.write(next as i64)?;
                                Ok(chunk.len() as i64)
                            }
                            _ => Err(EINVAL),
                        }
                    }
                    Err(errno) => Err(errno),
                }
            }
            PreparedFilesystemCall::Create { .. }
            | PreparedFilesystemCall::Write { .. }
            | PreparedFilesystemCall::WriteAt { .. }
            | PreparedFilesystemCall::Remove { .. }
            | PreparedFilesystemCall::CreateDir { .. }
            | PreparedFilesystemCall::RemoveDir { .. }
            | PreparedFilesystemCall::CreateDirName { .. }
            | PreparedFilesystemCall::UnlinkAt { .. }
            | PreparedFilesystemCall::SetPermissions { .. }
            | PreparedFilesystemCall::SetFilePermissions { .. }
            | PreparedFilesystemCall::Rename { .. }
            | PreparedFilesystemCall::HardLink { .. }
            | PreparedFilesystemCall::Symlink { .. }
            | PreparedFilesystemCall::CreateHardLink { .. }
            | PreparedFilesystemCall::SetFileTime { .. }
            | PreparedFilesystemCall::RemoveName { .. }
            | PreparedFilesystemCall::RemoveDirName { .. }
            | PreparedFilesystemCall::SetLen { .. }
            | PreparedFilesystemCall::SetFileTimes { .. }
            | PreparedFilesystemCall::ChangeOwner { .. }
            | PreparedFilesystemCall::ChangeOwnerNoFollow { .. }
            | PreparedFilesystemCall::ChangeFileOwner { .. } => {
                self.record_grant_refusal(0, true, crate::FilesystemGrantRefusalReason::OutsideGrantedRoots);
                Err(EACCES)
            },
            PreparedFilesystemCall::Canonicalize { .. }
            | PreparedFilesystemCall::FindFirst { .. }
            | PreparedFilesystemCall::FindNext { .. }
            | PreparedFilesystemCall::FindClose { .. }
            | PreparedFilesystemCall::OpenPathHandle { .. }
            | PreparedFilesystemCall::CloseHandle { .. }
            | PreparedFilesystemCall::GetOsfHandle { .. }
            | PreparedFilesystemCall::FinalPathNameByHandle { .. }
            | PreparedFilesystemCall::LockFileEx { .. }
            | PreparedFilesystemCall::UnlockFile { .. }
            | PreparedFilesystemCall::GetLastError
            | PreparedFilesystemCall::ReadMetadata { .. }
            | PreparedFilesystemCall::ReadFileMetadata { .. }
            | PreparedFilesystemCall::ReadSymlinkMetadata { .. }
            | PreparedFilesystemCall::Sync { .. }
            | PreparedFilesystemCall::SyncData { .. }
            | PreparedFilesystemCall::LockFile { .. }
            | PreparedFilesystemCall::Errno => return Err(Halt::Unsupported(
                "captured inputs expose only logical source operations, not native metadata, host paths, handles, locks or synchronization".into()
            )),
        };
        Ok(Value::Int(match self.hosted_errno(outcome) {
            Some(value) => value,
            None => -1,
        }))
    }

    fn read_captured(
        &mut self,
        descriptor: i32,
        buffer: &crate::interpreter::evaluator::PreparedByteOutput,
        count: u64,
        offset: Option<u64>,
    ) -> EvalResult<Result<i64, i32>> {
        let real =
            real_fs(self).ok_or_else(|| Halt::Trap("captured provider disappeared".into()))?;
        let Some(entry) = real.captured_descriptors.get(&descriptor) else {
            return Ok(Err(EBADF));
        };
        let Some(open) = entry.open.as_ref() else {
            return Ok(Err(EBADF));
        };
        let Some(description) = real.captured_descriptions.get(&open.description) else {
            return Ok(Err(EBADF));
        };
        let position = offset.unwrap_or(description.cursor);
        let result = real.captured_snapshot(entry.root).and_then(|snapshot| {
            snapshot
                .read(&open.relative, position, count)
                .map_err(snapshot_errno)
        });
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(errno) => return Ok(Err(errno)),
        };
        buffer.write(bytes)?;
        let length = bytes.len() as u64;
        let description_identity = open.description;
        if offset.is_none()
            && let Some(description) = self
                .real_fs_mut()
                .captured_descriptions
                .get_mut(&description_identity)
        {
            description.cursor = position + length;
        }
        Ok(Ok(length as i64))
    }
}
