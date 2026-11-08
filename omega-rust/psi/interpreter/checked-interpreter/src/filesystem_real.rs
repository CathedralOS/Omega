/*
    Scoped and unscoped physical host filesystem execution and captured-input
    root serving for the checked-tree interpreter.

    We canonicalize and authorize every path against `FsGrants` before touching
    the host OS, enforce descriptor write bits and staging-sponsor limits, and
    serve immutable captured source roots without host path fallback.
*/

use crate::interpreter::evaluator::{EvalResult, Evaluator, PreparedFilesystemCall, Value};
use crate::{
    CanonicalFilesystemMetadataRowKind, FilesystemGrantRootIdentity,
    FilesystemMetadataObservationKind,
};
use authorized_paths::canonical_relative_path;
use grants::{Grants, canonical_grants};
use host_io::{real_os_bytes, real_path};
use metadata::host_metadata_matches_canonical_kind;

use std::collections::BTreeMap;

use std::path::{Path, PathBuf};

fn io_errno(error: &std::io::Error) -> i32 {
    error.raw_os_error().unwrap_or(5)
}

#[cfg(not(unix))]
const ENOTSUP: i32 = 45;
const EBADF: i32 = 9;
const EACCES: i32 = 13;
const EINVAL: i32 = 22;
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;
const MAX_FILESYSTEM_OBSERVATION_PATH_BYTES: usize = 16 * 1024 * 1024;

struct RealFd {
    file: std::fs::File,
    path: PathBuf,
    sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
    append: bool,
    write_granted: bool,
    canonical_metadata: Option<CanonicalFilesystemMetadataRowKind>,
}

pub(crate) struct RealFs {
    files: BTreeMap<i32, RealFd>,
    next_fd: i32,
    captured_roots: BTreeMap<FilesystemGrantRootIdentity, source::SourceSnapshot>,
    captured_descriptors: BTreeMap<i32, captured::CapturedDescriptor>,
    captured_descriptions: BTreeMap<i32, captured::CapturedOpenDescription>,
    pub(crate) errno: i32,
    grants: Option<Grants>,
    sponsor: Option<crate::FilesystemSponsor>,
}

impl RealFs {
    pub(crate) fn new(
        grants: Option<crate::FsGrants>,
        sponsor: Option<crate::FilesystemSponsor>,
    ) -> Result<Self, String> {
        let grants = grants.map(canonical_grants).transpose()?;
        Ok(Self {
            files: BTreeMap::new(),
            next_fd: 3,
            captured_roots: BTreeMap::new(),
            captured_descriptors: BTreeMap::new(),
            captured_descriptions: BTreeMap::new(),
            errno: 0,
            grants,
            sponsor,
        })
    }

    pub(crate) fn with_captured_inputs(
        grants: crate::FsGrants,
        sponsor: Option<crate::FilesystemSponsor>,
        inputs: Vec<crate::CapturedFilesystemRoot>,
    ) -> Result<Self, String> {
        let mut filesystem = Self::new(Some(grants), sponsor)?;
        for input in inputs {
            if filesystem
                .grants
                .as_ref()
                .is_some_and(|grants| grants.root(input.identity).is_some())
                || filesystem
                    .captured_roots
                    .insert(input.identity, input.snapshot)
                    .is_some()
            {
                return Err(format!(
                    "filesystem root identity {} is duplicated across provider storage",
                    input.identity.get()
                ));
            }
        }
        Ok(filesystem)
    }

    fn allocate_descriptor(&mut self) -> Result<i32, i32> {
        let descriptor = self.next_fd;
        self.next_fd = descriptor.checked_add(1).ok_or(24)?;
        Ok(descriptor)
    }

    pub(crate) fn descriptor_root(&self, descriptor: i32) -> Option<FilesystemGrantRootIdentity> {
        self.captured_descriptors
            .get(&descriptor)
            .map(|entry| entry.root)
            .or_else(|| {
                self.files.get(&descriptor).and_then(|entry| {
                    self.grants
                        .as_ref()?
                        .matching_root(&entry.path, false)
                        .map(|root| root.identity)
                })
            })
    }

    pub(crate) fn prepare_rooted_path(
        &self,
        identity: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> Option<crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemPath>
    {
        if self.captured_roots.contains_key(&identity) {
            return Some(
                crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemPath::Captured {
                    root: identity,
                    relative: relative.to_vec(),
                },
            );
        }
        self.rooted_path_bytes(identity, relative).map(
            crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemPath::Native,
        )
    }

    pub(crate) fn require_sealed_rooted_file(
        &self,
        root: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> EvalResult<()> {
        use crate::interpreter::evaluator::halts::filesystem_sponsor_halt;
        let sponsor = self.sponsor.as_ref().ok_or_else(|| {
            crate::interpreter::evaluator::Halt::Trap(
                "generated source requires sponsored output custody".into(),
            )
        })?;
        let path = self
            .rooted_path_bytes(root, relative)
            .and_then(|bytes| real_path(&bytes))
            .ok_or_else(|| {
                crate::interpreter::evaluator::Halt::Trap(
                    "generated source names no physical output root".into(),
                )
            })?;
        let bound = match sponsor.bind_path(&path) {
            Ok(bound) => bound,
            Err(error) => return filesystem_sponsor_halt(error),
        };
        if let Err(error) = sponsor.require_sealed(&bound) {
            return filesystem_sponsor_halt(error);
        }
        match sponsor.entry(&bound) {
            Ok(Some(crate::FilesystemSponsorEntry::Object { .. })) => Ok(()),
            Ok(_) => Err(crate::interpreter::evaluator::Halt::Trap(
                "generated source must name a sealed regular file".into(),
            )),
            Err(error) => filesystem_sponsor_halt(error),
        }
    }

    pub(crate) fn call_uses_captured_input(
        &self,
        call: &PreparedFilesystemCall,
        plan: &crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemLogicalHandlePlan,
    ) -> bool {
        call.has_captured_path()
            || plan.inputs.iter().any(|input| {
                i32::try_from(input.raw)
                    .ok()
                    .is_some_and(|descriptor| self.captured_descriptors.contains_key(&descriptor))
            })
    }

    pub(crate) fn is_scoped(&self) -> bool {
        self.grants.is_some()
    }

    fn rooted_path_bytes(
        &self,
        identity: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> Option<Vec<u8>> {
        let root = self.grants.as_ref()?.root(identity)?;
        let relative = real_path(relative)?;
        real_os_bytes(root.path.join(relative).as_os_str())
    }

    fn insert(
        &mut self,
        file: std::fs::File,
        path: PathBuf,
        sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
        append: bool,
    ) -> Result<i64, i32> {
        let canonical_metadata = self.canonical_metadata_for_path(&path)?;
        if let Some(kind) = canonical_metadata {
            let metadata = file.metadata().map_err(|error| io_errno(&error))?;
            if !host_metadata_matches_canonical_kind(&metadata, kind) {
                return Err(EACCES);
            }
        }
        self.insert_preselected(file, path, sponsor_descriptor, append, canonical_metadata)
    }

    fn insert_preselected(
        &mut self,
        file: std::fs::File,
        path: PathBuf,
        sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
        append: bool,
        canonical_metadata: Option<CanonicalFilesystemMetadataRowKind>,
    ) -> Result<i64, i32> {
        let write_granted = self
            .grants
            .as_ref()
            .is_none_or(|grants| grants.matching_root(&path, true).is_some());
        let fd = self.allocate_descriptor()?;
        self.files.insert(
            fd,
            RealFd {
                file,
                path,
                sponsor_descriptor,
                append,
                write_granted,
                canonical_metadata,
            },
        );
        Ok(i64::from(fd))
    }

    fn canonical_metadata_for_path(
        &self,
        path: &Path,
    ) -> Result<Option<CanonicalFilesystemMetadataRowKind>, i32> {
        let Some(grants) = self.grants.as_ref() else {
            return Ok(None);
        };
        let Some(root) = grants.matching_root(path, false) else {
            return Err(EACCES);
        };
        let Some(index) = root.canonical_metadata.as_ref() else {
            return Ok(None);
        };
        let relative = path
            .strip_prefix(&root.path)
            .ok()
            .and_then(canonical_relative_path)
            .ok_or(EACCES)?;
        index.row(&relative).map(Some).ok_or(EACCES)
    }

    fn descriptor_write_granted(&self, fd: i32) -> Option<bool> {
        self.files.get(&fd).map(|entry| entry.write_granted)
    }
}

impl Drop for RealFs {
    fn drop(&mut self) {
        let Some(sponsor) = self.sponsor.clone() else {
            return;
        };
        while let Some(fd) = self.files.keys().next().copied() {
            let descriptor = self
                .files
                .get(&fd)
                .and_then(|entry| entry.sponsor_descriptor);
            let prepared = descriptor
                .as_ref()
                .and_then(|descriptor| sponsor.prepare_close(descriptor).ok());
            self.files.remove(&fd);
            if let Some(prepared) = prepared {
                let _ = prepared.commit();
            }
        }
    }
}

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(crate) fn try_real_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
        let result: i64 = crate::prepared_filesystem_call_dispatch!(
                    self, call;
                    literal {
        PreparedFilesystemCall::Open { path, flags } => {
                        self.real_open_prepared(path.into_native()?, flags, 0, false)?
                    }
        PreparedFilesystemCall::CreateDir { path }
                    | PreparedFilesystemCall::CreateDirName { name: path } => {
                        match self.authorized_namespace_leaf(&path, true, 0) {
                        Some(path) => {
                            if let Some(sponsor) = self.real_fs_mut().sponsor.clone() {
                                let bound = sponsor::sponsor_value(sponsor.bind_path(&path))?;
                                if matches!(
                                    sponsor::sponsor_value(sponsor.entry(&bound))?,
                                    Some(crate::FilesystemSponsorEntry::Directory)
                                ) {
                                    return Ok(Value::Int(0));
                                }
                            }
                            let prepared = self.prepare_sponsored_create_directory(&path)?;
                            let outcome = std::fs::create_dir(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                        None => -1,
                        }
                    },
        PreparedFilesystemCall::ReadMetadata { path, buffer } => {
                        self.real_read_metadata(
                        path.into_native()?,
                        &buffer,
                        FilesystemMetadataObservationKind::FollowedPath,
                    )?
                    }
                    PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } => {
                        self.real_read_metadata(
                            path.into_native()?,
                            &buffer,
                            FilesystemMetadataObservationKind::UnfollowedFinalPath,
                        )?
                    }
                    PreparedFilesystemCall::Errno => { i64::from(self.real_fs_mut().errno) }
                    PreparedFilesystemCall::GetLastError => { i64::from(self.real_fs_mut().errno) }
                    PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid } => {
                        self.real_change_owner_no_follow(path.into_native()?, uid, gid)
                    }
                    }
                    dispatch {
                        Create => real_create,
                        OpenCreate => real_open_create,
                        OpenPathHandle => real_open_path_handle,
                        Read => real_read,
                        Write => real_write,
                        Seek => real_seek,
                        Close => real_close,
                        CloseHandle => real_close_handle,
                        Duplicate => real_duplicate,
                        SetLen => real_set_len,
                        Sync | SyncData => real_sync,
                        Remove | RemoveName => real_remove,
                        RemoveDir | RemoveDirName => real_remove_dir,
                        Rename => real_rename,
                        ReadAt => real_read_at,
                        WriteAt => real_write_at,
                        ReadDir => real_read_dir,
                        FindFirst => real_find_first,
                        FindNext => real_find_next,
                        FindClose => real_find_close,
                        ReadFileMetadata => real_read_file_metadata,
                        Canonicalize => real_canonicalize,
                        HardLink => real_hard_link,
                        CreateHardLink => real_create_hard_link,
                        GetOsfHandle => real_get_osf_handle,
                        FinalPathNameByHandle => real_final_path_name_by_handle,
                        SetFileTime => real_set_file_time,
                        Symlink => real_symlink,
                        ReadLink => real_read_link,
                        SetPermissions => real_set_permissions,
                        SetFilePermissions => real_set_file_permissions,
                        SetFileTimes => real_set_file_times,
                        LockFile => real_lock_file,
                        LockFileEx => real_lock_file_ex,
                        UnlockFile => real_unlock_file,
                        ChangeOwner => real_change_owner,
                        ChangeFileOwner => real_change_file_owner,
                        UnlinkAt => real_unlink_at,
                        OpenAt => real_open_at
                    }
                );
        Ok(Value::Int(result))
    }

    fn real_open_prepared(
        &mut self,
        path: Vec<u8>,
        flags: i32,
        mode: u32,
        create_variant: bool,
    ) -> EvalResult<i64> {
        let wants_write =
            crate::interpreter::evaluator::filesystem::host_open_flags::wants_write(flags);
        match self.authorized_path(&path, wants_write, 0) {
            Some(path) => {
                self.open_flags_and_finish(path, flags, mode, create_variant, wants_write)
            }
            None => Ok(-1),
        }
    }

    fn real_fs_mut(&mut self) -> &mut RealFs {
        self.real_fs
            .as_mut()
            .expect("try_real_filesystem_call only runs in real mode")
    }

    pub(crate) fn hosted_errno<T>(&mut self, outcome: Result<T, i32>) -> Option<T> {
        match outcome {
            Ok(value) => Some(value),
            Err(errno) => {
                self.real_fs_mut().errno = errno;
                None
            }
        }
    }

    #[cfg(unix)]
    fn real_result_unit(&mut self, outcome: std::io::Result<()>) -> i64 {
        match outcome {
            Ok(()) => 0,
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                -1
            }
        }
    }
}

pub(crate) fn real_fs<'a>(evaluator: &'a Evaluator<'_>) -> Option<&'a RealFs> {
    evaluator.real_fs.as_ref()
}

pub(crate) mod authorized_paths {
    use super::host_io::real_path;
    use super::{EACCES, ENOENT, MAX_FILESYSTEM_OBSERVATION_PATH_BYTES, real_fs};
    use crate::interpreter::evaluator::{
        FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    };
    use crate::{FilesystemAuthorizedPath, FilesystemGrantRootIdentity};
    use std::path::{Path, PathBuf};

    fn resolve_parent_for_check(path: &Path) -> Option<PathBuf> {
        let parent = path.parent()?;
        let name = path.file_name()?;
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".").canonicalize().ok()?
        } else {
            parent.canonicalize().ok()?
        };
        Some(parent.join(name))
    }

    fn resolve_for_check(path: &Path) -> Option<PathBuf> {
        if let Ok(canonical) = path.canonicalize() {
            return Some(canonical);
        }
        const LINK_LIMIT: usize = 41;
        let mut current = path.to_path_buf();
        for _ in 0..LINK_LIMIT {
            let Ok(target) = std::fs::read_link(&current) else {
                return resolve_parent_for_check(&current);
            };
            current = if target.is_absolute() {
                target
            } else {
                current.parent()?.join(target)
            };
            if let Ok(canonical) = current.canonicalize() {
                return Some(canonical);
            }
        }
        None
    }

    pub(crate) fn canonical_relative_path(path: &Path) -> Option<Vec<u8>> {
        let mut encoded = Vec::new();
        for component in path.components() {
            let std::path::Component::Normal(component) = component else {
                return None;
            };
            let component = component.to_str()?.as_bytes();
            if !encoded.is_empty() {
                encoded.push(b'/');
            }
            encoded.extend_from_slice(component);
        }
        Some(encoded)
    }

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        pub(crate) fn authorized_path(
            &mut self,
            path_bytes: &[u8],
            write: bool,
            operand_ordinal: u8,
        ) -> Option<PathBuf> {
            self.authorized_path_with_follow(path_bytes, write, true, operand_ordinal)
        }

        pub(crate) fn authorized_path_no_follow(
            &mut self,
            path_bytes: &[u8],
            write: bool,
            operand_ordinal: u8,
        ) -> Option<PathBuf> {
            self.authorized_path_with_follow(path_bytes, write, false, operand_ordinal)
        }

        pub(crate) fn authorized_namespace_leaf(
            &mut self,
            path_bytes: &[u8],
            write: bool,
            operand_ordinal: u8,
        ) -> Option<PathBuf> {
            self.authorized_path_no_follow(path_bytes, write, operand_ordinal)
        }

        pub(crate) fn authorized_path_with_follow(
            &mut self,
            path_bytes: &[u8],
            write: bool,
            follow: bool,
            operand_ordinal: u8,
        ) -> Option<PathBuf> {
            let Some(path) = real_path(path_bytes) else {
                self.real_fs_mut().errno = EACCES;
                self.record_grant_refusal(
                    operand_ordinal,
                    write,
                    FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                );
                return None;
            };
            self.authorized_native_path(&path, write, follow, operand_ordinal)
        }

        pub(crate) fn authorized_native_path(
            &mut self,
            path: &Path,
            write: bool,
            follow: bool,
            operand_ordinal: u8,
        ) -> Option<PathBuf> {
            let Some(grants) = real_fs(self).and_then(|filesystem| filesystem.grants.as_ref())
            else {
                return Some(path.to_path_buf());
            };
            let resolved = if follow {
                resolve_for_check(path)
            } else {
                resolve_parent_for_check(path)
            };
            let Some(resolved) = resolved else {
                self.real_fs_mut().errno = ENOENT;
                self.record_grant_refusal(
                    operand_ordinal,
                    write,
                    FilesystemGrantRefusalReason::Unresolvable,
                );
                return None;
            };
            if let Some(root) = grants.matching_root(&resolved, write) {
                let relative_path = resolved
                    .strip_prefix(&root.path)
                    .ok()
                    .and_then(canonical_relative_path);
                let Some(relative_path) = relative_path else {
                    self.real_fs_mut().errno = EACCES;
                    self.record_grant_refusal(
                        operand_ordinal,
                        write,
                        FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                    );
                    return None;
                };
                let root = root.identity;
                if !self.record_authorized_path(operand_ordinal, write, root, relative_path) {
                    self.real_fs_mut().errno = EACCES;
                    return None;
                }
                Some(resolved)
            } else {
                self.real_fs_mut().errno = EACCES;
                self.record_grant_refusal(
                    operand_ordinal,
                    write,
                    FilesystemGrantRefusalReason::OutsideGrantedRoots,
                );
                None
            }
        }

        pub(crate) fn record_authorized_path(
            &mut self,
            operand_ordinal: u8,
            write: bool,
            root: FilesystemGrantRootIdentity,
            relative_path: Vec<u8>,
        ) -> bool {
            let Some(next_total) = self
                .filesystem_observation_path_bytes
                .checked_add(relative_path.len())
                .filter(|total| *total <= MAX_FILESYSTEM_OBSERVATION_PATH_BYTES)
            else {
                self.record_grant_refusal(
                    operand_ordinal,
                    write,
                    FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded,
                );
                self.filesystem_observation_resource_halt = Some(format!(
                    "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_PATH_BYTES}-byte rooted-path ceiling"
                ));
                return false;
            };
            let Some(attempt_index) = self.filesystem_operation_attempt_stack.last().copied()
            else {
                return false;
            };
            self.filesystem_observation_path_bytes = next_total;
            self.filesystem_operation_attempts[attempt_index]
                .authorized_paths
                .push(FilesystemAuthorizedPath::new(
                    operand_ordinal,
                    if write {
                        FilesystemGrantAccess::Write
                    } else {
                        FilesystemGrantAccess::Read
                    },
                    root,
                    relative_path,
                ));
            true
        }

        pub(crate) fn record_grant_refusal(
            &mut self,
            operand_ordinal: u8,
            write: bool,
            reason: FilesystemGrantRefusalReason,
        ) {
            let Some(attempt_index) = self.filesystem_operation_attempt_stack.last().copied()
            else {
                return;
            };
            self.filesystem_operation_attempts[attempt_index]
                .grant_refusals
                .push(FilesystemGrantRefusal::new(
                    operand_ordinal,
                    if write {
                        FilesystemGrantAccess::Write
                    } else {
                        FilesystemGrantAccess::Read
                    },
                    reason,
                ));
        }
    }

    #[cfg(all(test, unix))]
    pub(crate) mod confinement_tests {
        use super::super::grants::canonical_grants;
        use super::resolve_for_check;
        use std::os::unix::fs::symlink;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new(label: &str) -> Self {
                let identity = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "omega-confinement-{label}-{}-{identity}",
                    std::process::id()
                ));
                std::fs::create_dir(&path).expect("create confinement test directory");
                Self(path)
            }

            fn canonical(&self) -> PathBuf {
                self.0.canonicalize().expect("test directory resolves")
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn write_root(identity: u32, path: PathBuf) -> crate::FsGrants {
            crate::FsGrants {
                read_roots: Vec::new(),
                write_roots: vec![crate::FilesystemGrantRoot::new(
                    crate::FilesystemGrantRootIdentity::new(identity)
                        .expect("nonzero grant identity"),
                    path,
                )],
            }
        }

        #[test]
        fn a_dangling_link_leaf_resolves_to_its_target_and_leaves_the_write_root() {
            let inside = TestDirectory::new("dangling-inside");
            let outside = TestDirectory::new("dangling-outside");
            let link = inside.0.join("x");
            let escape = outside.0.join("new");
            symlink(&escape, &link).expect("authorize only the link name");
            assert!(!escape.exists(), "the link target starts absent");

            let grants =
                canonical_grants(write_root(1, inside.0.clone())).expect("the write root resolves");

            let resolved = resolve_for_check(&link).expect("the target's parent exists");
            assert_eq!(
                resolved,
                outside.canonical().join("new"),
                "resolution reports the location the open would actually reach"
            );
            assert!(
                grants.matching_root(&resolved, true).is_none(),
                "the link target is outside every grant"
            );
            assert!(
                !escape.exists(),
                "nothing is created outside the write root"
            );
        }

        #[test]
        fn a_dangling_link_leaf_inside_the_write_root_stays_authorized() {
            let inside = TestDirectory::new("dangling-in-root");
            let link = inside.0.join("x");
            symlink("y", &link).expect("authorize only the link name");

            let grants =
                canonical_grants(write_root(1, inside.0.clone())).expect("the write root resolves");

            let resolved = resolve_for_check(&link).expect("the link's own parent exists");
            assert_eq!(resolved, inside.canonical().join("y"));
            assert_eq!(
                grants
                    .matching_root(&resolved, true)
                    .map(|root| root.identity.get()),
                Some(1)
            );
        }

        #[test]
        fn a_dangling_link_cycle_is_unresolvable() {
            let directory = TestDirectory::new("dangling-cycle");
            symlink("second", directory.0.join("first")).expect("link one");
            symlink("first", directory.0.join("second")).expect("link two");

            assert_eq!(resolve_for_check(&directory.0.join("first")), None);
        }
    }
}

pub(crate) mod captured {
    use super::{EACCES, EBADF, EINVAL, ENOENT, ENOTDIR, RealFs, real_fs};
    use crate::FilesystemGrantRootIdentity;
    use crate::interpreter::evaluator::filesystem::preparation::PreparedFilesystemPath;
    use crate::interpreter::evaluator::{EvalResult, Halt, PreparedFilesystemCall, Value};
    use source::{SourceSnapshot, SourceSnapshotEntry, SourceSnapshotError};

    pub(crate) struct CapturedDescriptor {
        pub(crate) root: FilesystemGrantRootIdentity,
        open: Option<CapturedOpenDescriptor>,
    }

    struct CapturedOpenDescriptor {
        relative: Vec<u8>,
        description: i32,
    }

    pub(crate) struct CapturedOpenDescription {
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
        fn captured_snapshot(
            &self,
            root: FilesystemGrantRootIdentity,
        ) -> Result<&SourceSnapshot, i32> {
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
                return Err(45);
            }
            match self
                .captured_snapshot(root)?
                .lookup(&relative)
                .map_err(snapshot_errno)?
            {
                SourceSnapshotEntry::File(_) => {}
                SourceSnapshotEntry::Directory => return Err(21),
                SourceSnapshotEntry::Symlink(_) => return Err(40),
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

        pub(crate) fn serve_captured_filesystem_call(
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
            Ok(Value::Int(self.hosted_errno(outcome).unwrap_or(-1)))
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
}

pub(crate) mod descriptor_calls {
    use super::host_io::{
        positioned_read, positioned_write, real_lock, real_lock_win32, real_os_bytes,
        win32_error_code,
    };
    use super::metadata::SelectedFilesystemMetadata;
    use super::sponsor::checked_written_count;
    use super::{EBADF, io_errno};
    use crate::FilesystemMetadataObservationKind;
    use crate::interpreter::evaluator::filesystem::real::RealFs;
    use crate::interpreter::evaluator::{EvalResult, PreparedFilesystemCall, synthetic_handle_fd};
    use std::io::{Read, Seek, SeekFrom, Write};

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        pub(crate) fn real_read(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Read { fd, buffer, count } = call else {
                unreachable!("dispatched real_read")
            };
            Ok({
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => {
                            let mut bytes = vec![0u8; count.host];
                            match entry.file.read(&mut bytes) {
                                Ok(n) => {
                                    bytes.truncate(n);
                                    Ok(bytes)
                                }
                                Err(error) => Err(io_errno(&error)),
                            }
                        }
                        None => Err(EBADF),
                    }
                };
                match self.hosted_errno(outcome) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        n
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_write(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Write { fd, bytes } = call else {
                unreachable!("dispatched real_write")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let write = self.prepare_sponsored_write(fd, bytes.len(), None)?;
                let Some(prepared) = self.hosted_errno(write) else {
                    return Ok(-1);
                };
                let real = self.real_fs_mut();
                let outcome = match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.write(&bytes) {
                        Ok(n) => Ok(n),
                        Err(error) => Err(io_errno(&error)),
                    },
                    None => Err(EBADF),
                };
                match self.hosted_errno(outcome) {
                    Some(written) => {
                        self.commit_sponsored_write(prepared, written)?;
                        checked_written_count(written)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_seek(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Seek { fd, offset, whence } = call else {
                unreachable!("dispatched real_seek")
            };
            Ok({
                let position = match whence {
                    1 => SeekFrom::Current(offset),
                    2 => SeekFrom::End(offset),
                    _ => SeekFrom::Start(offset.max(0) as u64),
                };
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.seek(position) {
                        Ok(new_position) => new_position as i64,
                        Err(error) => {
                            real.errno = io_errno(&error);
                            -1
                        }
                    },
                    None => missing_descriptor(real),
                }
            })
        }

        pub(crate) fn real_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Close { fd } = call else {
                unreachable!("dispatched real_close")
            };
            Ok({
                if self.real_fs_mut().files.contains_key(&fd) {
                    let prepared = self.prepare_sponsored_close(fd)?;
                    self.real_fs_mut().files.remove(&fd);
                    self.commit_sponsored_mutation(prepared)?;
                    0
                } else {
                    self.real_fs_mut().errno = EBADF;
                    -1
                }
            })
        }

        pub(crate) fn real_close_handle(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::CloseHandle { handle } = call else {
                unreachable!("dispatched real_close_handle")
            };
            Ok({
                match synthetic_handle_fd(handle) {
                    Some(handle) if self.real_fs_mut().files.contains_key(&handle) => {
                        let prepared = self.prepare_sponsored_close(handle)?;
                        self.real_fs_mut().files.remove(&handle);
                        self.commit_sponsored_mutation(prepared)?;
                        1
                    }
                    _ => {
                        self.real_fs_mut().errno = 6;
                        0
                    }
                }
            })
        }

        pub(crate) fn real_duplicate(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Duplicate { fd } = call else {
                unreachable!("dispatched real_duplicate")
            };
            Ok({
                let prepared = self.prepare_sponsored_duplicate(fd)?;
                let cloned = match self.real_fs_mut().files.get(&fd) {
                    Some(entry) => entry
                        .file
                        .try_clone()
                        .map(|file| {
                            (
                                file,
                                entry.path.clone(),
                                entry.append,
                                entry.canonical_metadata,
                            )
                        })
                        .map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match self.hosted_errno(cloned) {
                    Some((file, path, append, canonical_metadata)) => {
                        let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                        self.real_fs_mut()
                            .insert_preselected(
                                file,
                                path,
                                sponsor_descriptor,
                                append,
                                canonical_metadata,
                            )
                            .unwrap_or_else(|errno| {
                                self.real_fs_mut().errno = errno;
                                -1
                            })
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_set_len(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::SetLen { fd, length } = call else {
                unreachable!("dispatched real_set_len")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let length = u64::try_from(length.max(0)).expect("nonnegative i64 fits in u64");
                let prepared = self.prepare_sponsored_set_extent(fd, length)?;
                let outcome = match self.real_fs_mut().files.get_mut(&fd) {
                    Some(entry) => entry.file.set_len(length).map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match self.hosted_errno(outcome) {
                    Some(()) => {
                        self.commit_sponsored_mutation(prepared)?;
                        0
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_sync(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let (PreparedFilesystemCall::Sync { fd } | PreparedFilesystemCall::SyncData { fd }) =
                call
            else {
                unreachable!("dispatched real_sync")
            };
            Ok({
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.sync_all() {
                        Ok(()) => 0,
                        Err(error) => {
                            real.errno = io_errno(&error);
                            -1
                        }
                    },
                    None => missing_descriptor(real),
                }
            })
        }

        pub(crate) fn real_read_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::ReadAt {
                fd,
                buffer,
                count,
                offset,
            } = call
            else {
                unreachable!("dispatched real_read_at")
            };
            Ok({
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => positioned_read(&mut entry.file, offset, count.host)
                            .map_err(|error| io_errno(&error)),
                        None => Err(EBADF),
                    }
                };
                match self.hosted_errno(outcome) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        n
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_write_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::WriteAt { fd, bytes, offset } = call else {
                unreachable!("dispatched real_write_at")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let write = self.prepare_sponsored_write(fd, bytes.len(), Some(offset))?;
                let Some(prepared) = self.hosted_errno(write) else {
                    return Ok(-1);
                };
                let outcome = match self.real_fs_mut().files.get_mut(&fd) {
                    Some(entry) => match positioned_write(&mut entry.file, offset, &bytes) {
                        Ok(n) => Ok(n),
                        Err(error) => Err(io_errno(&error)),
                    },
                    None => Err(EBADF),
                };
                match self.hosted_errno(outcome) {
                    Some(written) => {
                        self.commit_sponsored_write(prepared, written)?;
                        checked_written_count(written)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_read_file_metadata(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::ReadFileMetadata { fd, buffer } = call else {
                unreachable!("dispatched real_read_file_metadata")
            };
            Ok({
                let looked_up = match self.real_fs_mut().files.get(&fd) {
                    Some(entry) => match entry.canonical_metadata {
                        Some(metadata) => Ok(SelectedFilesystemMetadata::Canonical(metadata)),
                        None => entry
                            .file
                            .metadata()
                            .map(SelectedFilesystemMetadata::Physical)
                            .map_err(|error| io_errno(&error)),
                    },
                    None => Err(EBADF),
                };
                match self.hosted_errno(looked_up) {
                    Some(SelectedFilesystemMetadata::Canonical(metadata)) => {
                        self.write_canonical_fs_stat(
                            &buffer,
                            FilesystemMetadataObservationKind::OpenDescriptor,
                            metadata,
                        )?;
                        0
                    }
                    Some(SelectedFilesystemMetadata::Physical(metadata)) => {
                        self.write_real_fs_stat(
                            &buffer,
                            FilesystemMetadataObservationKind::OpenDescriptor,
                            &metadata,
                        )?;
                        0
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_get_osf_handle(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::GetOsfHandle { fd } = call else {
                unreachable!("dispatched real_get_osf_handle")
            };
            Ok({
                if self.real_fs_mut().files.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            })
        }

        pub(crate) fn real_final_path_name_by_handle(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::FinalPathNameByHandle {
                handle,
                buffer,
                capacity,
            } = call
            else {
                unreachable!("dispatched real_final_path_name_by_handle")
            };
            Ok({
                let path = synthetic_handle_fd(handle).and_then(|handle| {
                    self.real_fs_mut()
                        .files
                        .get(&handle)
                        .map(|entry| entry.path.clone())
                });
                match path {
                    Some(path) => match std::fs::canonicalize(path) {
                        Ok(path) => {
                            let Some(path) = real_os_bytes(path.as_os_str()) else {
                                self.real_fs_mut().errno = 1113;
                                return Ok(0);
                            };
                            if path.len() < capacity.host {
                                let mut bytes = path.clone();
                                bytes.push(0);
                                buffer.write(&bytes)?;
                                path.len() as i64
                            } else {
                                (path.len() + 1) as i64
                            }
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = win32_error_code(&error);
                            0
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = 6;
                        0
                    }
                }
            })
        }

        pub(crate) fn real_set_file_time(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::SetFileTime {
                handle,
                last_write: write_ft,
            } = call
            else {
                unreachable!("dispatched real_set_file_time")
            };
            Ok({
                let filetime = write_ft
                    .get(0..8)
                    .and_then(|s| <[u8; 8]>::try_from(s).ok())
                    .map(i64::from_le_bytes)
                    .unwrap_or(0);
                let secs = filetime / 10_000_000 - 11_644_473_600;
                let Some(handle) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6;
                    return Ok(0);
                };
                if !self.require_real_descriptor_write_grant(handle, true) {
                    return Ok(0);
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&handle) {
                    Some(entry) => {
                        let stamp = if secs >= 0 {
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
                        } else {
                            std::time::UNIX_EPOCH
                        };
                        match entry.file.set_modified(stamp) {
                            Ok(()) => 1,
                            Err(error) => {
                                real.errno = win32_error_code(&error);
                                0
                            }
                        }
                    }
                    None => {
                        real.errno = 6;
                        0
                    }
                }
            })
        }

        pub(crate) fn real_set_file_permissions(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::SetFilePermissions { fd, mode } = call else {
                unreachable!("dispatched real_set_file_permissions")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            match entry
                                .file
                                .set_permissions(std::fs::Permissions::from_mode(mode & 0o7777))
                            {
                                Ok(()) => 0,
                                Err(error) => {
                                    real.errno = io_errno(&error);
                                    -1
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (entry, mode);
                            real.errno = super::ENOTSUP;
                            -1
                        }
                    }
                    None => missing_descriptor(real),
                }
            })
        }

        pub(crate) fn real_set_file_times(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::SetFileTimes { fd, times } = call else {
                unreachable!("dispatched real_set_file_times")
            };
            Ok({
                let mtime_secs = times
                    .bytes
                    .get(16..24)
                    .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
                    .unwrap_or(0);
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => {
                        let stamp = if mtime_secs >= 0 {
                            std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(mtime_secs as u64)
                        } else {
                            std::time::UNIX_EPOCH
                        };
                        match entry.file.set_modified(stamp) {
                            Ok(()) => 0,
                            Err(error) => {
                                real.errno = io_errno(&error);
                                -1
                            }
                        }
                    }
                    None => missing_descriptor(real),
                }
            })
        }

        pub(crate) fn real_lock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::LockFile { fd, operation } = call else {
                unreachable!("dispatched real_lock_file")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock(&entry.file, operation, &mut real.errno),
                    None => missing_descriptor(real),
                }
            })
        }

        pub(crate) fn real_lock_file_ex(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::LockFileEx { handle, flags } = call else {
                unreachable!("dispatched real_lock_file_ex")
            };
            Ok({
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6;
                    return Ok(0);
                };
                if !self.require_real_descriptor_write_grant(fd, true) {
                    return Ok(0);
                }
                let flags = flags as i32;
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock_win32(&entry.file, flags, &mut real.errno),
                    None => {
                        real.errno = 6;
                        0
                    }
                }
            })
        }

        pub(crate) fn real_unlock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::UnlockFile { handle } = call else {
                unreachable!("dispatched real_unlock_file")
            };
            Ok({
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6;
                    return Ok(0);
                };
                if !self.require_real_descriptor_write_grant(fd, true) {
                    return Ok(0);
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => match entry.file.unlock() {
                        Ok(()) => 1,
                        Err(error) => {
                            real.errno = error.raw_os_error().unwrap_or(158);
                            0
                        }
                    },
                    None => {
                        real.errno = 6;
                        0
                    }
                }
            })
        }

        pub(crate) fn real_change_file_owner(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } = call else {
                unreachable!("dispatched real_change_file_owner")
            };
            Ok({
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(-1);
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => {
                        #[cfg(unix)]
                        {
                            let owner = (uid >= 0).then_some(uid as u32);
                            let group = (gid >= 0).then_some(gid as u32);
                            match std::os::unix::fs::fchown(&entry.file, owner, group) {
                                Ok(()) => 0,
                                Err(error) => {
                                    real.errno = io_errno(&error);
                                    -1
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (entry, uid, gid);
                            real.errno = super::ENOTSUP;
                            -1
                        }
                    }
                    None => missing_descriptor(real),
                }
            })
        }
    }

    fn missing_descriptor(real: &mut RealFs) -> i64 {
        real.errno = EBADF;
        -1
    }
}

pub(crate) mod directory_calls {
    use super::host_io::{DirectoryEntrySnapshotKind, real_directory_entries};
    use super::{EBADF, ENOENT};
    use crate::interpreter::evaluator::{EvalResult, PreparedFilesystemCall, dirent_record_chunk};

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        pub(crate) fn real_read_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
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
                match self.hosted_errno(listed) {
                    Some(entries) => {
                        let records = crate::interpreter::evaluator::pack_dirent_records(&entries)?;
                        let start = position.initial.max(0) as usize;
                        let (chunk, next_position) =
                            dirent_record_chunk(&records, start, count.host);
                        if chunk.is_empty() {
                            0
                        } else {
                            let n = chunk.len();
                            buffer.write(chunk)?;
                            position.write(next_position as i64)?;
                            n as i64
                        }
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_find_first(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::FindFirst { pattern, data } = call else {
                unreachable!("dispatched real_find_first")
            };
            Ok({
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
                match self.hosted_errno(listed) {
                    Some(entries) => {
                        let mut queue: std::collections::VecDeque<(Vec<u8>, bool)> = entries
                            .into_iter()
                            .map(|(name, d_type)| (name, d_type == 4))
                            .collect();
                        let (name, is_dir) =
                            queue.pop_front().expect("dot entries are always present");
                        self.write_find_data(&data, &name, is_dir)?;
                        let handle = self.virtual_next_find;
                        self.virtual_next_find += 1;
                        self.virtual_finds.insert(handle, queue);
                        handle
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_find_next(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::FindNext { handle, data } = call else {
                unreachable!("dispatched real_find_next")
            };
            self.pop_virtual_find(handle, &data)
        }

        pub(crate) fn real_find_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::FindClose { handle } = call else {
                unreachable!("dispatched real_find_close")
            };
            Ok(self.close_virtual_find(handle))
        }
    }
}

pub(crate) mod grants {
    use crate::{CanonicalFilesystemMetadataIndex, FilesystemGrantRootIdentity};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    pub(crate) struct Grants {
        pub(crate) read_roots: Vec<GrantRoot>,
        pub(crate) write_roots: Vec<GrantRoot>,
    }

    #[derive(Debug)]
    pub(crate) struct GrantRoot {
        pub(crate) identity: FilesystemGrantRootIdentity,
        pub(crate) path: PathBuf,
        pub(crate) canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
    }

    pub(crate) fn canonical_root(root: &Path) -> Result<PathBuf, String> {
        root.canonicalize().map_err(|error| {
            format!(
                "filesystem grant root `{}` cannot be resolved: {error}",
                root.display()
            )
        })
    }

    impl Grants {
        pub(crate) fn matching_root(&self, resolved: &Path, write: bool) -> Option<&GrantRoot> {
            self.write_roots
                .iter()
                .chain(if write {
                    [].iter()
                } else {
                    self.read_roots.as_slice().iter()
                })
                .filter(|root| resolved.starts_with(&root.path))
                .max_by(|left, right| {
                    left.path
                        .components()
                        .count()
                        .cmp(&right.path.components().count())
                })
        }

        pub(crate) fn root(&self, identity: FilesystemGrantRootIdentity) -> Option<&GrantRoot> {
            self.write_roots
                .iter()
                .chain(self.read_roots.iter())
                .find(|root| root.identity == identity)
        }
    }

    pub(crate) fn canonical_grants(grants: crate::FsGrants) -> Result<Grants, String> {
        let mut identities = BTreeSet::new();
        let mut physical_roots = BTreeMap::<PathBuf, FilesystemGrantRootIdentity>::new();
        let mut canonicalize = |root: crate::FilesystemGrantRoot,
                                write: bool|
         -> Result<GrantRoot, String> {
            if write && root.canonical_metadata().is_some() {
                return Err(format!(
                    "writable filesystem grant root `{}` cannot carry immutable canonical metadata",
                    root.identity().get()
                ));
            }
            if !identities.insert(root.identity()) {
                return Err(format!(
                    "filesystem grant-root identity `{}` is duplicated",
                    root.identity().get()
                ));
            }
            let path = canonical_root(root.path())?;
            if let Some(previous) = physical_roots.insert(path.clone(), root.identity()) {
                return Err(format!(
                    "filesystem grant root `{}` has conflicting identities `{}` and `{}`",
                    path.display(),
                    previous.get(),
                    root.identity().get()
                ));
            }
            Ok(GrantRoot {
                identity: root.identity(),
                path,
                canonical_metadata: root.canonical_metadata().cloned(),
            })
        };
        let read_roots = grants
            .read_roots
            .into_iter()
            .map(|root| canonicalize(root, false))
            .collect::<Result<Vec<_>, _>>()?;
        let write_roots = grants
            .write_roots
            .into_iter()
            .map(|root| canonicalize(root, true))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Grants {
            read_roots,
            write_roots,
        })
    }
}

pub(crate) mod host_io {
    use super::{EINVAL, ENOTDIR, io_errno};
    use crate::interpreter::evaluator::filesystem::host_open_flags::{
        o_append, o_creat, o_excl, o_trunc,
    };
    use crate::interpreter::evaluator::{
        EvalResult, checked_directory_name_snapshot_total, checked_directory_record_snapshot_total,
    };
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::path::{Path, PathBuf};

    pub(crate) fn real_path(bytes: &[u8]) -> Option<PathBuf> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
        }
        #[cfg(not(unix))]
        {
            std::str::from_utf8(bytes).ok().map(PathBuf::from)
        }
    }

    pub(crate) fn real_os_byte_slice(value: &std::ffi::OsStr) -> Option<&[u8]> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Some(value.as_bytes())
        }
        #[cfg(not(unix))]
        {
            value.to_str().map(str::as_bytes)
        }
    }

    pub(crate) fn real_os_bytes(value: &std::ffi::OsStr) -> Option<Vec<u8>> {
        real_os_byte_slice(value).map(<[u8]>::to_vec)
    }

    #[derive(Clone, Copy)]
    pub(crate) enum DirectoryEntrySnapshotKind {
        PackedRecords,
        FindCursor,
    }

    pub(crate) fn real_directory_entries(
        path: &Path,
        snapshot_kind: DirectoryEntrySnapshotKind,
    ) -> EvalResult<Result<Vec<(Vec<u8>, u8)>, i32>> {
        match std::fs::metadata(path) {
            Ok(metadata) if !metadata.is_dir() => return Ok(Err(ENOTDIR)),
            Ok(_) => {}
            Err(error) => return Ok(Err(io_errno(&error))),
        }
        let (mut name_bytes, mut record_bytes) = match snapshot_kind {
            DirectoryEntrySnapshotKind::PackedRecords => {
                let record_bytes = checked_directory_record_snapshot_total(0, 1)?;
                (
                    None,
                    Some(checked_directory_record_snapshot_total(record_bytes, 2)?),
                )
            }
            DirectoryEntrySnapshotKind::FindCursor => {
                let name_bytes = checked_directory_name_snapshot_total(0, 1)?;
                (
                    Some(checked_directory_name_snapshot_total(name_bytes, 2)?),
                    None,
                )
            }
        };
        let mut children: Vec<(Vec<u8>, u8)> = Vec::new();
        let listing = match std::fs::read_dir(path) {
            Ok(listing) => listing,
            Err(error) => return Ok(Err(io_errno(&error))),
        };
        for dir_entry in listing {
            let dir_entry = match dir_entry {
                Ok(dir_entry) => dir_entry,
                Err(error) => return Ok(Err(io_errno(&error))),
            };
            let d_type = match dir_entry.file_type() {
                Ok(kind) if kind.is_dir() => 4,
                Ok(kind) if kind.is_symlink() => 10,
                Ok(_) => 8,
                Err(_) => 0,
            };
            let file_name = dir_entry.file_name();
            let Some(bytes) = real_os_byte_slice(&file_name) else {
                return Ok(Err(EINVAL));
            };
            let bytes = crate::interpreter::evaluator::portable_directory_entry_name(bytes);
            if let Some(current) = name_bytes {
                name_bytes = Some(checked_directory_name_snapshot_total(current, bytes.len())?);
            }
            if let Some(current) = record_bytes {
                record_bytes = Some(checked_directory_record_snapshot_total(
                    current,
                    bytes.len(),
                )?);
            }
            children.push((bytes.to_vec(), d_type));
        }
        children.sort();
        let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
        entries.extend(children);
        Ok(Ok(entries))
    }

    pub(crate) fn positioned_read(
        file: &mut std::fs::File,
        offset: i64,
        count: usize,
    ) -> std::io::Result<Vec<u8>> {
        let saved = file.stream_position()?;
        let offset = u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64");
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0u8; count];
        let outcome = file.read(&mut buffer);
        file.seek(SeekFrom::Start(saved))?;
        let n = outcome?;
        buffer.truncate(n);
        Ok(buffer)
    }

    pub(crate) fn positioned_write(
        file: &mut std::fs::File,
        offset: i64,
        bytes: &[u8],
    ) -> std::io::Result<usize> {
        let saved = file.stream_position()?;
        let offset = u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64");
        file.seek(SeekFrom::Start(offset))?;
        let outcome = file.write(bytes);
        file.seek(SeekFrom::Start(saved))?;
        outcome
    }

    pub(crate) fn open_real(
        options: &std::fs::OpenOptions,
        path: &Path,
        wants_write: bool,
    ) -> std::io::Result<std::fs::File> {
        #[cfg(windows)]
        if !wants_write && path.is_dir() {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            return std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path);
        }
        #[cfg(not(windows))]
        let _ = wants_write;
        options.open(path)
    }

    pub(crate) fn open_options_for(
        flags: i32,
        mode: u32,
        apply_creation_mode: bool,
    ) -> std::fs::OpenOptions {
        let access = flags & 0x3;
        let mut options = std::fs::OpenOptions::new();
        options
            .read(access == 0 || access == 2)
            .write(access == 1 || access == 2)
            .append(o_append(flags))
            .truncate(o_trunc(flags))
            .create(o_creat(flags));
        if o_creat(flags) && o_excl(flags) {
            options.create_new(true);
        }
        #[cfg(unix)]
        if o_creat(flags) && apply_creation_mode {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode & 0o7777);
        }
        #[cfg(not(unix))]
        let _ = (mode, apply_creation_mode);
        options
    }

    pub(crate) fn real_lock(file: &std::fs::File, operation: i32, errno: &mut i32) -> i64 {
        const EWOULDBLOCK: i32 = 35;
        let non_blocking = operation & 4 != 0;
        let outcome = if operation & 8 != 0 {
            file.unlock()
        } else if operation & 2 != 0 {
            if non_blocking {
                match file.try_lock() {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        *errno = EWOULDBLOCK;
                        return -1;
                    }
                }
            } else {
                file.lock()
            }
        } else if operation & 1 != 0 {
            if non_blocking {
                match file.try_lock_shared() {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        *errno = EWOULDBLOCK;
                        return -1;
                    }
                }
            } else {
                file.lock_shared()
            }
        } else {
            *errno = 22;
            return -1;
        };
        match outcome {
            Ok(()) => 0,
            Err(error) => {
                *errno = io_errno(&error);
                -1
            }
        }
    }

    pub(crate) fn real_lock_win32(file: &std::fs::File, flags: i32, last_error: &mut i32) -> i64 {
        let immediate = flags & 1 != 0;
        let exclusive = flags & 2 != 0;
        if immediate {
            let outcome = if exclusive {
                file.try_lock()
            } else {
                file.try_lock_shared()
            };
            return match outcome {
                Ok(()) => 1,
                Err(std::fs::TryLockError::WouldBlock) => {
                    *last_error = 33;
                    0
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    *last_error = error.raw_os_error().unwrap_or(1);
                    0
                }
            };
        }
        let outcome = if exclusive {
            file.lock()
        } else {
            file.lock_shared()
        };
        match outcome {
            Ok(()) => 1,
            Err(error) => {
                *last_error = error.raw_os_error().unwrap_or(1);
                0
            }
        }
    }

    pub(crate) fn win32_error_code(error: &std::io::Error) -> i32 {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::NotFound => 2,
            ErrorKind::PermissionDenied => 5,
            ErrorKind::AlreadyExists => 183,
            ErrorKind::WouldBlock => 33,
            _ => error.raw_os_error().unwrap_or(1),
        }
    }
}

pub(crate) mod metadata {
    use super::EACCES;
    use super::io_errno;
    use super::real_fs;
    use crate::interpreter::evaluator::{EvalResult, PreparedByteOutput};
    use crate::{CanonicalFilesystemMetadataRowKind, FilesystemMetadataObservationKind};

    pub(crate) enum SelectedFilesystemMetadata {
        Canonical(CanonicalFilesystemMetadataRowKind),
        Physical(std::fs::Metadata),
    }

    pub(crate) fn canonical_metadata_values(
        metadata: CanonicalFilesystemMetadataRowKind,
    ) -> (u32, i64, i64) {
        let (mode, size) = match metadata {
            CanonicalFilesystemMetadataRowKind::Directory => (0o040555, 0),
            CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length,
            } => (
                if executable { 0o100555 } else { 0o100444 },
                logical_byte_length as i64,
            ),
            CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => (0o120777, target_spelling_logical_byte_length as i64),
        };
        (mode, size, 1_000_000_000)
    }

    pub(crate) fn host_metadata_matches_canonical_kind(
        metadata: &std::fs::Metadata,
        canonical: CanonicalFilesystemMetadataRowKind,
    ) -> bool {
        match canonical {
            CanonicalFilesystemMetadataRowKind::Directory => metadata.is_dir(),
            CanonicalFilesystemMetadataRowKind::File { .. } => metadata.is_file(),
            CanonicalFilesystemMetadataRowKind::Symlink { .. } => metadata.file_type().is_symlink(),
        }
    }

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        pub(crate) fn real_read_metadata(
            &mut self,
            path: Vec<u8>,
            buffer: &PreparedByteOutput,
            kind: FilesystemMetadataObservationKind,
        ) -> EvalResult<i64> {
            let no_follow = kind == FilesystemMetadataObservationKind::UnfollowedFinalPath;
            let authorized = if no_follow {
                self.authorized_path_no_follow(&path, false, 0)
            } else {
                self.authorized_path(&path, false, 0)
            };
            let Some(path) = authorized else {
                return Ok(-1);
            };
            let looked_up = if no_follow {
                std::fs::symlink_metadata(&path)
            } else {
                std::fs::metadata(&path)
            };
            match looked_up {
                Ok(metadata) => {
                    let selected = real_fs(self)
                        .expect("real metadata calls have a real provider")
                        .canonical_metadata_for_path(&path);
                    match selected {
                        Ok(Some(canonical))
                            if host_metadata_matches_canonical_kind(&metadata, canonical) =>
                        {
                            self.write_canonical_fs_stat(buffer, kind, canonical)?;
                        }
                        Ok(Some(_)) | Err(_) => {
                            self.real_fs_mut().errno = EACCES;
                            return Ok(-1);
                        }
                        Ok(None) => self.write_real_fs_stat(buffer, kind, &metadata)?,
                    }
                    Ok(0)
                }
                Err(error) => {
                    self.real_fs_mut().errno = io_errno(&error);
                    Ok(-1)
                }
            }
        }

        pub(crate) fn real_change_owner_no_follow(
            &mut self,
            path: Vec<u8>,
            uid: i32,
            gid: i32,
        ) -> i64 {
            let Some(path) = self.authorized_path_no_follow(&path, true, 0) else {
                return -1;
            };
            #[cfg(unix)]
            {
                let owner = (uid >= 0).then_some(uid as u32);
                let group = (gid >= 0).then_some(gid as u32);
                self.real_result_unit(std::os::unix::fs::lchown(path, owner, group))
            }
            #[cfg(not(unix))]
            {
                let _ = (path, uid, gid);
                self.real_fs_mut().errno = super::ENOTSUP;
                -1
            }
        }
        pub(crate) fn write_real_fs_stat(
            &mut self,
            output: &PreparedByteOutput,
            kind: FilesystemMetadataObservationKind,
            metadata: &std::fs::Metadata,
        ) -> EvalResult<()> {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                let type_bits = if metadata.is_dir() {
                    0o040000
                } else if metadata.file_type().is_symlink() {
                    0o120000
                } else {
                    0o100000
                };
                (type_bits | (metadata.permissions().mode() & 0o7777)) as u16
            };
            #[cfg(not(unix))]
            let mode: u16 = if metadata.is_dir() {
                0o040000
            } else {
                0o100000
            };
            let size = metadata.len() as i64;
            let mtime_secs = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(0);
            self.write_fs_stat(output, kind, u32::from(mode), size, mtime_secs)
        }

        pub(crate) fn write_canonical_fs_stat(
            &mut self,
            output: &PreparedByteOutput,
            kind: FilesystemMetadataObservationKind,
            metadata: CanonicalFilesystemMetadataRowKind,
        ) -> EvalResult<()> {
            let (mode, size, modification_time) = canonical_metadata_values(metadata);
            self.write_fs_stat(output, kind, mode, size, modification_time)
        }
    }
}

pub(crate) mod path_calls {
    use super::host_io::{open_real, real_os_bytes, real_path};
    use super::{EACCES, EBADF, EINVAL, ENOENT, io_errno};
    use crate::interpreter::evaluator::filesystem::host_open_flags;
    use crate::interpreter::evaluator::{
        EvalResult, FilesystemGrantRefusalReason, PreparedFilesystemCall,
    };

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        pub(crate) fn real_create(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Create { path } = call else {
                unreachable!("dispatched real_create")
            };
            Ok({
                match self.authorized_path(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(&path, true, true, true)?;
                        let mut options = std::fs::OpenOptions::new();
                        options.write(true).create(true).truncate(true);
                        if matches!(prepared, super::sponsor::SponsorPreparation::Prepared(_)) {
                            options.create_new(true);
                        }
                        let opened = options.open(&path);
                        self.finish_real_open(opened, path, false, prepared, false)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_open_create(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::OpenCreate { path, flags, mode } = call else {
                unreachable!("dispatched real_open_create")
            };
            Ok({
                let wants_write = host_open_flags::wants_write(flags);
                match self.authorized_path(&path, wants_write, 0) {
                    Some(path) => {
                        self.open_flags_and_finish(path, flags, mode as u32, true, wants_write)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_open_path_handle(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::OpenPathHandle { path, .. } = call else {
                unreachable!("dispatched real_open_path_handle")
            };
            Ok({
                match self.authorized_path(&path, false, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(&path, false, false, false)?;
                        let mut options = std::fs::OpenOptions::new();
                        options.read(true);
                        let opened = open_real(&options, &path, false);
                        self.finish_real_open(opened, path, false, prepared, true)?
                    }
                    None => {
                        let real = self.real_fs_mut();
                        real.errno = if real.errno == ENOENT { 2 } else { 5 };
                        -1
                    }
                }
            })
        }

        pub(crate) fn real_remove(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let (PreparedFilesystemCall::Remove { path }
            | PreparedFilesystemCall::RemoveName { path }) = call
            else {
                unreachable!("dispatched real_remove")
            };
            Ok({
                match self.authorized_namespace_leaf(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        let outcome = std::fs::remove_file(path);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_remove_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let (PreparedFilesystemCall::RemoveDir { path }
            | PreparedFilesystemCall::RemoveDirName { path }) = call
            else {
                unreachable!("dispatched real_remove_dir")
            };
            Ok({
                match self.authorized_namespace_leaf(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        let outcome = std::fs::remove_dir(path);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_rename(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Rename { from, to } = call else {
                unreachable!("dispatched real_rename")
            };
            Ok({
                match (
                    self.authorized_namespace_leaf(&from, true, 0),
                    self.authorized_namespace_leaf(&to, true, 1),
                ) {
                    (Some(from), Some(to)) => {
                        let prepared = self.prepare_sponsored_rename(&from, &to)?;
                        let outcome = std::fs::rename(from, to);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    _ => -1,
                }
            })
        }

        pub(crate) fn real_canonicalize(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::Canonicalize { path, buffer } = call else {
                unreachable!("dispatched real_canonicalize")
            };
            Ok({
                match self.authorized_path(&path, false, 0) {
                    Some(path) => match std::fs::canonicalize(&path) {
                        Ok(resolved) => {
                            let Some(path_bytes) = real_os_bytes(resolved.as_os_str()) else {
                                self.real_fs_mut().errno = EINVAL;
                                return Ok(0);
                            };
                            let mut bytes = path_bytes.clone();
                            bytes.push(0);
                            buffer.write(&bytes)?;
                            1
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = io_errno(&error);
                            0
                        }
                    },
                    None => 0,
                }
            })
        }

        pub(crate) fn real_hard_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::HardLink { original, link } = call else {
                unreachable!("dispatched real_hard_link")
            };
            Ok({
                match (
                    self.authorized_namespace_leaf(&original, true, 0),
                    self.authorized_namespace_leaf(&link, true, 1),
                ) {
                    (Some(original), Some(link)) => {
                        let prepared = self.prepare_sponsored_hard_link(&original, &link)?;
                        let outcome = std::fs::hard_link(original, link);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    _ => -1,
                }
            })
        }

        pub(crate) fn real_create_hard_link(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::CreateHardLink { link, existing } = call else {
                unreachable!("dispatched real_create_hard_link")
            };
            Ok({
                match (
                    self.authorized_namespace_leaf(&existing, true, 1),
                    self.authorized_namespace_leaf(&link, true, 0),
                ) {
                    (Some(existing), Some(link)) => {
                        let prepared = self.prepare_sponsored_hard_link(&existing, &link)?;
                        let outcome = std::fs::hard_link(existing, link);
                        self.finish_real_mutation(outcome, prepared, true)?
                    }
                    _ => 0,
                }
            })
        }

        pub(crate) fn real_symlink(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::Symlink { target, link } = call else {
                unreachable!("dispatched real_symlink")
            };
            Ok({
                match self.authorized_namespace_leaf(&link, true, 1) {
                    Some(link) => {
                        let prepared = self.prepare_sponsored_symlink(&link, &target)?;
                        #[cfg(unix)]
                        {
                            let outcome = std::os::unix::fs::symlink(
                                real_path(&target).expect("unix path bytes are lossless"),
                                link,
                            );
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (target, link, prepared);
                            self.real_fs_mut().errno = super::ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_read_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::ReadLink {
                path,
                buffer,
                count,
            } = call
            else {
                unreachable!("dispatched real_read_link")
            };
            Ok({
                match self.authorized_path_no_follow(&path, false, 0) {
                    Some(path) => match std::fs::read_link(&path) {
                        Ok(target) => {
                            let Some(bytes) = real_os_bytes(target.as_os_str()) else {
                                self.real_fs_mut().errno = EINVAL;
                                return Ok(-1);
                            };
                            let n = bytes.len().min(count.host);
                            buffer.write(&bytes[..n])?;
                            n as i64
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = io_errno(&error);
                            -1
                        }
                    },
                    None => -1,
                }
            })
        }

        pub(crate) fn real_set_permissions(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::SetPermissions { path, mode } = call else {
                unreachable!("dispatched real_set_permissions")
            };
            Ok({
                match self.authorized_path(&path, true, 0) {
                    Some(path) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            self.real_result_unit(std::fs::set_permissions(
                                path,
                                std::fs::Permissions::from_mode(mode & 0o7777),
                            ))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (path, mode);
                            self.real_fs_mut().errno = super::ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_change_owner(
            &mut self,
            call: PreparedFilesystemCall,
        ) -> EvalResult<i64> {
            let PreparedFilesystemCall::ChangeOwner { path, uid, gid } = call else {
                unreachable!("dispatched real_change_owner")
            };
            Ok({
                let authorized = self.authorized_path(&path, true, 0);
                match authorized {
                    Some(path) => {
                        #[cfg(unix)]
                        {
                            let owner = (uid >= 0).then_some(uid as u32);
                            let group = (gid >= 0).then_some(gid as u32);
                            let outcome = std::os::unix::fs::chown(path, owner, group);
                            self.real_result_unit(outcome)
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (path, uid, gid);
                            self.real_fs_mut().errno = super::ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_unlink_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } = call else {
                unreachable!("dispatched real_unlink_at")
            };
            Ok({
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => match real_path(&name) {
                        Some(name) => entry.path.join(name),
                        None => {
                            self.real_fs_mut().errno = EACCES;
                            self.record_grant_refusal(
                                1,
                                true,
                                FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                            );
                            return Ok(-1);
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(-1);
                    }
                };
                match self.authorized_native_path(&joined, true, false, 1) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        if flags & 0x80 != 0 {
                            let outcome = std::fs::remove_dir(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        } else {
                            let outcome = std::fs::remove_file(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                    }
                    None => -1,
                }
            })
        }

        pub(crate) fn real_open_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
            let PreparedFilesystemCall::OpenAt { dirfd, name, flags } = call else {
                unreachable!("dispatched real_open_at")
            };
            Ok({
                let wants_write = host_open_flags::wants_write(flags);
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => match real_path(&name) {
                        Some(name) => entry.path.join(name),
                        None => {
                            self.real_fs_mut().errno = EACCES;
                            self.record_grant_refusal(
                                1,
                                wants_write,
                                FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                            );
                            return Ok(-1);
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(-1);
                    }
                };
                match self.authorized_native_path(&joined, wants_write, true, 1) {
                    Some(path) => self.open_flags_and_finish(path, flags, 0, false, wants_write)?,
                    None => -1,
                }
            })
        }
    }
}

pub(crate) mod sponsor {
    use super::host_io::{open_options_for, open_real, win32_error_code};
    use super::{EACCES, EBADF, io_errno, real_fs};
    use crate::FilesystemSponsorError;
    use crate::interpreter::evaluator::filesystem::host_open_flags::{o_append, o_creat, o_trunc};
    use crate::interpreter::evaluator::{EvalResult, filesystem_sponsor_halt};
    use std::io::Seek;
    use std::path::{Path, PathBuf};

    pub(crate) fn sponsor_value<T>(result: Result<T, FilesystemSponsorError>) -> EvalResult<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => filesystem_sponsor_halt(error),
        }
    }

    pub(crate) fn checked_written_count(written: usize) -> EvalResult<i64> {
        match i64::try_from(written) {
            Ok(written) => Ok(written),
            Err(_) => filesystem_sponsor_halt(FilesystemSponsorError::ArithmeticOverflow),
        }
    }

    #[derive(Debug)]
    pub(crate) enum SponsorPreparation<T> {
        Unsponsored,
        Prepared(T),
    }

    pub(crate) fn sponsor_preparation<T>(
        result: Result<T, FilesystemSponsorError>,
    ) -> EvalResult<SponsorPreparation<T>> {
        sponsor_value(result).map(SponsorPreparation::Prepared)
    }

    pub(crate) fn read_only_open_bypasses_sponsor(
        path: &Path,
        session_root: &Path,
        may_create: bool,
        truncates: bool,
        wants_write: bool,
    ) -> bool {
        (!path.starts_with(session_root) || path == session_root)
            && !wants_write
            && !may_create
            && !truncates
    }

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        fn sponsor_context(
            &mut self,
            fd: i32,
        ) -> Option<(crate::FilesystemSponsor, crate::FilesystemOpenDescriptor)> {
            let real = self.real_fs_mut();
            real.sponsor.clone().and_then(|sponsor| {
                real.files
                    .get(&fd)
                    .and_then(|entry| entry.sponsor_descriptor)
                    .map(|descriptor| (sponsor, descriptor))
            })
        }

        pub(crate) fn require_real_descriptor_write_grant(
            &mut self,
            fd: i32,
            win32_errors: bool,
        ) -> bool {
            match real_fs(self).and_then(|filesystem| filesystem.descriptor_write_granted(fd)) {
                Some(true) => true,
                Some(false) => {
                    self.real_fs_mut().errno = if win32_errors { 5 } else { EACCES };
                    false
                }
                None => {
                    self.real_fs_mut().errno = if win32_errors { 6 } else { EBADF };
                    false
                }
            }
        }

        pub(crate) fn prepare_sponsored_open(
            &mut self,
            path: &Path,
            may_create: bool,
            truncates: bool,
            wants_write: bool,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
            let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone())
            else {
                return Ok(SponsorPreparation::Unsponsored);
            };
            let session_root = sponsor_value(sponsor.session_root())?;
            if read_only_open_bypasses_sponsor(
                path,
                &session_root,
                may_create,
                truncates,
                wants_write,
            ) {
                return Ok(SponsorPreparation::Unsponsored);
            }
            let sponsored_path = sponsor_value(sponsor.bind_path(path))?;
            if !wants_write && !may_create && !truncates {
                sponsor_value(sponsor.require_sealed(&sponsored_path))?;
                return Ok(SponsorPreparation::Unsponsored);
            }
            if !may_create {
                return sponsor_value(Err(FilesystemSponsorError::UnsupportedStagingOperation(
                    "write-open of an existing file",
                )));
            }
            sponsor_preparation(sponsor.prepare_create_object_open(&sponsored_path, 0))
        }

        pub(crate) fn finish_real_open(
            &mut self,
            opened: std::io::Result<std::fs::File>,
            path: PathBuf,
            append: bool,
            prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
            win32_errors: bool,
        ) -> EvalResult<i64> {
            match opened {
                Ok(file) => {
                    let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                    match self
                        .real_fs_mut()
                        .insert(file, path, sponsor_descriptor, append)
                    {
                        Ok(fd) => Ok(fd),
                        Err(errno) => {
                            self.real_fs_mut().errno = if win32_errors { 5 } else { errno };
                            Ok(-1)
                        }
                    }
                }
                Err(error) => {
                    self.real_fs_mut().errno = if win32_errors {
                        win32_error_code(&error)
                    } else {
                        io_errno(&error)
                    };
                    Ok(-1)
                }
            }
        }

        pub(crate) fn open_flags_and_finish(
            &mut self,
            path: PathBuf,
            flags: i32,
            mode: u32,
            apply_creation_mode: bool,
            wants_write: bool,
        ) -> EvalResult<i64> {
            let prepared =
                self.prepare_sponsored_open(&path, o_creat(flags), o_trunc(flags), wants_write)?;
            let mut options = open_options_for(flags, mode, apply_creation_mode);
            if matches!(prepared, SponsorPreparation::Prepared(_)) {
                options.create_new(true);
            }
            let opened = open_real(&options, &path, wants_write);
            self.finish_real_open(opened, path, o_append(flags), prepared, false)
        }

        pub(crate) fn commit_sponsored_open(
            &mut self,
            prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
        ) -> EvalResult<Option<crate::FilesystemOpenDescriptor>> {
            match prepared {
                SponsorPreparation::Unsponsored => Ok(None),
                SponsorPreparation::Prepared(prepared) => {
                    Ok(Some(sponsor_value(prepared.commit())?))
                }
            }
        }

        pub(crate) fn prepare_sponsored_duplicate(
            &mut self,
            fd: i32,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
            if self.sponsor_context(fd).is_none() {
                return Ok(SponsorPreparation::Unsponsored);
            }
            Self::refuse_staged("duplicate writer")
        }

        pub(crate) fn prepare_sponsored_close(
            &mut self,
            fd: i32,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            let Some((sponsor, descriptor)) = self.sponsor_context(fd) else {
                return Ok(SponsorPreparation::Unsponsored);
            };
            sponsor_preparation(sponsor.prepare_close(&descriptor))
        }

        pub(crate) fn prepare_sponsored_set_extent(
            &mut self,
            fd: i32,
            _extent: u64,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            if self.sponsor_context(fd).is_none() {
                return Ok(SponsorPreparation::Unsponsored);
            }
            Self::refuse_staged("truncate writer")
        }

        pub(crate) fn prepare_sponsored_write(
            &mut self,
            fd: i32,
            requested_bytes: usize,
            positioned_offset: Option<i64>,
        ) -> EvalResult<Result<SponsorPreparation<crate::PreparedFilesystemWrite>, i32>> {
            let real = self.real_fs_mut();
            let Some(sponsor) = real.sponsor.clone() else {
                return Ok(Ok(SponsorPreparation::Unsponsored));
            };
            let Some(entry) = real.files.get_mut(&fd) else {
                return Ok(Err(EBADF));
            };
            let Some(descriptor) = entry.sponsor_descriptor else {
                return Ok(Ok(SponsorPreparation::Unsponsored));
            };
            let offset = if entry.append {
                match entry.file.metadata() {
                    Ok(metadata) => metadata.len(),
                    Err(error) => return Ok(Err(io_errno(&error))),
                }
            } else if let Some(offset) = positioned_offset {
                u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64")
            } else {
                match entry.file.stream_position() {
                    Ok(offset) => offset,
                    Err(error) => return Ok(Err(io_errno(&error))),
                }
            };
            let requested_bytes = match u64::try_from(requested_bytes) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return filesystem_sponsor_halt(FilesystemSponsorError::ArithmeticOverflow);
                }
            };
            Ok(Ok(sponsor_preparation(sponsor.prepare_write(
                &descriptor,
                offset,
                requested_bytes,
            ))?))
        }

        pub(crate) fn commit_sponsored_write(
            &mut self,
            prepared: SponsorPreparation<crate::PreparedFilesystemWrite>,
            written: usize,
        ) -> EvalResult<()> {
            let prepared = match prepared {
                SponsorPreparation::Unsponsored => return Ok(()),
                SponsorPreparation::Prepared(prepared) => prepared,
            };
            let written = match u64::try_from(written) {
                Ok(written) => written,
                Err(_) => {
                    return filesystem_sponsor_halt(FilesystemSponsorError::ArithmeticOverflow);
                }
            };
            sponsor_value(prepared.commit_written(written))
        }

        pub(crate) fn prepare_sponsored_create_directory(
            &mut self,
            path: &Path,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            let Some((sponsor, path)) = self.sponsor_path(path)? else {
                return Ok(SponsorPreparation::Unsponsored);
            };
            sponsor_preparation(sponsor.prepare_create_directory(&path))
        }

        pub(crate) fn prepare_sponsored_unlink(
            &mut self,
            path: &Path,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            if self.sponsor_path(path)?.is_none() {
                return Ok(SponsorPreparation::Unsponsored);
            }
            Self::refuse_staged("remove staging entry")
        }

        pub(crate) fn prepare_sponsored_rename(
            &mut self,
            from: &Path,
            to: &Path,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone())
            else {
                return Ok(SponsorPreparation::Unsponsored);
            };
            sponsor_value(sponsor.bind_path(from))?;
            sponsor_value(sponsor.bind_path(to))?;
            sponsor_value(Err(FilesystemSponsorError::UnsupportedStagingOperation(
                "rename staging entry",
            )))
        }

        pub(crate) fn prepare_sponsored_hard_link(
            &mut self,
            existing: &Path,
            new_name: &Path,
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone())
            else {
                return Ok(SponsorPreparation::Unsponsored);
            };
            sponsor_value(sponsor.bind_path(existing))?;
            sponsor_value(sponsor.bind_path(new_name))?;
            sponsor_value(Err(FilesystemSponsorError::UnsupportedStagingOperation(
                "hard link staging entry",
            )))
        }

        pub(crate) fn prepare_sponsored_symlink(
            &mut self,
            link: &Path,
            _target_spelling: &[u8],
        ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
            if self.sponsor_path(link)?.is_none() {
                return Ok(SponsorPreparation::Unsponsored);
            }
            Self::refuse_staged("symlink staging entry")
        }

        fn refuse_staged<T>(operation: &'static str) -> EvalResult<SponsorPreparation<T>> {
            sponsor_value(Err(FilesystemSponsorError::UnsupportedStagingOperation(
                operation,
            )))
        }

        pub(crate) fn sponsor_path(
            &mut self,
            path: &Path,
        ) -> EvalResult<Option<(crate::FilesystemSponsor, crate::FilesystemSponsorPath)>> {
            let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone())
            else {
                return Ok(None);
            };
            let path = sponsor_value(sponsor.bind_path(path))?;
            Ok(Some((sponsor, path)))
        }

        pub(crate) fn commit_sponsored_mutation(
            &mut self,
            prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
        ) -> EvalResult<()> {
            match prepared {
                SponsorPreparation::Unsponsored => Ok(()),
                SponsorPreparation::Prepared(prepared) => sponsor_value(prepared.commit()),
            }
        }

        pub(crate) fn finish_real_mutation(
            &mut self,
            outcome: std::io::Result<()>,
            prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
            win32_result: bool,
        ) -> EvalResult<i64> {
            match outcome {
                Ok(()) => {
                    self.commit_sponsored_mutation(prepared)?;
                    Ok(if win32_result { 1 } else { 0 })
                }
                Err(error) => {
                    self.real_fs_mut().errno = if win32_result {
                        win32_error_code(&error)
                    } else {
                        io_errno(&error)
                    };
                    Ok(if win32_result { 0 } else { -1 })
                }
            }
        }
    }
}
