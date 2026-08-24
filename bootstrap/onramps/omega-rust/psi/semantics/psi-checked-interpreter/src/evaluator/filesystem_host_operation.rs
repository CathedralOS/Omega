/// Closed identity of one canonical toolchain `FilesystemHost` requirement.
///
/// The explicit tags are reserved for the future build-observation transcript.
/// They do not grant authority: callers must first prove that the target symbol
/// belongs to the exact toolchain trait and source file. Aliases remain distinct
/// because a receipt must preserve the ABI operation that was actually invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub(super) enum FilesystemHostOperation {
    Create = 1,
    Open = 2,
    OpenCreate = 3,
    Read = 4,
    Write = 5,
    ReadAt = 6,
    WriteAt = 7,
    Close = 8,
    Remove = 9,
    Seek = 10,
    CreateDir = 11,
    RemoveDir = 12,
    CreateDirName = 13,
    OpenAt = 14,
    UnlinkAt = 15,
    SetPermissions = 16,
    SetFilePermissions = 17,
    Rename = 18,
    HardLink = 19,
    Symlink = 20,
    ReadLink = 21,
    Canonicalize = 22,
    ReadDir = 23,
    FindFirst = 24,
    FindNext = 25,
    FindClose = 26,
    CreateHardLink = 27,
    OpenPathHandle = 28,
    CloseHandle = 29,
    GetOsfHandle = 30,
    FinalPathNameByHandle = 31,
    SetFileTime = 32,
    LockFileEx = 33,
    UnlockFile = 34,
    GetLastError = 35,
    RemoveName = 36,
    RemoveDirName = 37,
    ReadMetadata = 38,
    ReadFileMetadata = 39,
    ReadSymlinkMetadata = 40,
    SetLen = 41,
    SetFileTimes = 42,
    Sync = 43,
    SyncData = 44,
    Duplicate = 45,
    LockFile = 46,
    ChangeOwner = 47,
    ChangeOwnerNoFollow = 48,
    ChangeFileOwner = 49,
    Errno = 50,
}

impl FilesystemHostOperation {
    #[cfg(test)]
    pub(super) const ALL: [Self; 50] = [
        Self::Create,
        Self::Open,
        Self::OpenCreate,
        Self::Read,
        Self::Write,
        Self::ReadAt,
        Self::WriteAt,
        Self::Close,
        Self::Remove,
        Self::Seek,
        Self::CreateDir,
        Self::RemoveDir,
        Self::CreateDirName,
        Self::OpenAt,
        Self::UnlinkAt,
        Self::SetPermissions,
        Self::SetFilePermissions,
        Self::Rename,
        Self::HardLink,
        Self::Symlink,
        Self::ReadLink,
        Self::Canonicalize,
        Self::ReadDir,
        Self::FindFirst,
        Self::FindNext,
        Self::FindClose,
        Self::CreateHardLink,
        Self::OpenPathHandle,
        Self::CloseHandle,
        Self::GetOsfHandle,
        Self::FinalPathNameByHandle,
        Self::SetFileTime,
        Self::LockFileEx,
        Self::UnlockFile,
        Self::GetLastError,
        Self::RemoveName,
        Self::RemoveDirName,
        Self::ReadMetadata,
        Self::ReadFileMetadata,
        Self::ReadSymlinkMetadata,
        Self::SetLen,
        Self::SetFileTimes,
        Self::Sync,
        Self::SyncData,
        Self::Duplicate,
        Self::LockFile,
        Self::ChangeOwner,
        Self::ChangeOwnerNoFollow,
        Self::ChangeFileOwner,
        Self::Errno,
    ];

    /// Convert a readable leaf only after exact canonical symbol/source
    /// identity has selected the filesystem authority.
    pub(super) fn from_canonical_name(name: &str) -> Option<Self> {
        Some(match name {
            "create" => Self::Create,
            "open" => Self::Open,
            "open_create" => Self::OpenCreate,
            "read" => Self::Read,
            "write" => Self::Write,
            "read_at" => Self::ReadAt,
            "write_at" => Self::WriteAt,
            "close" => Self::Close,
            "remove" => Self::Remove,
            "seek" => Self::Seek,
            "create_dir" => Self::CreateDir,
            "remove_dir" => Self::RemoveDir,
            "create_dir_name" => Self::CreateDirName,
            "open_at" => Self::OpenAt,
            "unlink_at" => Self::UnlinkAt,
            "set_permissions" => Self::SetPermissions,
            "set_file_permissions" => Self::SetFilePermissions,
            "rename" => Self::Rename,
            "hard_link" => Self::HardLink,
            "symlink" => Self::Symlink,
            "read_link" => Self::ReadLink,
            "canonicalize" => Self::Canonicalize,
            "read_dir" => Self::ReadDir,
            "find_first" => Self::FindFirst,
            "find_next" => Self::FindNext,
            "find_close" => Self::FindClose,
            "create_hard_link" => Self::CreateHardLink,
            "open_path_handle" => Self::OpenPathHandle,
            "close_handle" => Self::CloseHandle,
            "get_osfhandle" => Self::GetOsfHandle,
            "final_path_name_by_handle" => Self::FinalPathNameByHandle,
            "set_file_time" => Self::SetFileTime,
            "lock_file_ex" => Self::LockFileEx,
            "unlock_file" => Self::UnlockFile,
            "get_last_error" => Self::GetLastError,
            "remove_name" => Self::RemoveName,
            "remove_dir_name" => Self::RemoveDirName,
            "read_metadata" => Self::ReadMetadata,
            "read_file_metadata" => Self::ReadFileMetadata,
            "read_symlink_metadata" => Self::ReadSymlinkMetadata,
            "set_len" => Self::SetLen,
            "set_file_times" => Self::SetFileTimes,
            "sync" => Self::Sync,
            "sync_data" => Self::SyncData,
            "duplicate" => Self::Duplicate,
            "lock_file" => Self::LockFile,
            "change_owner" => Self::ChangeOwner,
            "change_owner_no_follow" => Self::ChangeOwnerNoFollow,
            "change_file_owner" => Self::ChangeFileOwner,
            "errno" => Self::Errno,
            _ => return None,
        })
    }

    pub(super) const fn operation_tag(self) -> u16 {
        self as u16
    }

    pub(super) const fn canonical_name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Open => "open",
            Self::OpenCreate => "open_create",
            Self::Read => "read",
            Self::Write => "write",
            Self::ReadAt => "read_at",
            Self::WriteAt => "write_at",
            Self::Close => "close",
            Self::Remove => "remove",
            Self::Seek => "seek",
            Self::CreateDir => "create_dir",
            Self::RemoveDir => "remove_dir",
            Self::CreateDirName => "create_dir_name",
            Self::OpenAt => "open_at",
            Self::UnlinkAt => "unlink_at",
            Self::SetPermissions => "set_permissions",
            Self::SetFilePermissions => "set_file_permissions",
            Self::Rename => "rename",
            Self::HardLink => "hard_link",
            Self::Symlink => "symlink",
            Self::ReadLink => "read_link",
            Self::Canonicalize => "canonicalize",
            Self::ReadDir => "read_dir",
            Self::FindFirst => "find_first",
            Self::FindNext => "find_next",
            Self::FindClose => "find_close",
            Self::CreateHardLink => "create_hard_link",
            Self::OpenPathHandle => "open_path_handle",
            Self::CloseHandle => "close_handle",
            Self::GetOsfHandle => "get_osfhandle",
            Self::FinalPathNameByHandle => "final_path_name_by_handle",
            Self::SetFileTime => "set_file_time",
            Self::LockFileEx => "lock_file_ex",
            Self::UnlockFile => "unlock_file",
            Self::GetLastError => "get_last_error",
            Self::RemoveName => "remove_name",
            Self::RemoveDirName => "remove_dir_name",
            Self::ReadMetadata => "read_metadata",
            Self::ReadFileMetadata => "read_file_metadata",
            Self::ReadSymlinkMetadata => "read_symlink_metadata",
            Self::SetLen => "set_len",
            Self::SetFileTimes => "set_file_times",
            Self::Sync => "sync",
            Self::SyncData => "sync_data",
            Self::Duplicate => "duplicate",
            Self::LockFile => "lock_file",
            Self::ChangeOwner => "change_owner",
            Self::ChangeOwnerNoFollow => "change_owner_no_follow",
            Self::ChangeFileOwner => "change_file_owner",
            Self::Errno => "errno",
        }
    }

    /// Classify operations whose returned bytes may reveal an absolute host
    /// path. A rooted transcript must virtualize or reject such a result.
    #[cfg(test)]
    const fn path_result_exposure(self) -> PathResultExposure {
        match self {
            Self::ReadLink => PathResultExposure::MayBeAbsolute,
            Self::Canonicalize | Self::FinalPathNameByHandle => PathResultExposure::AlwaysAbsolute,
            _ => PathResultExposure::None,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathResultExposure {
    None,
    MayBeAbsolute,
    AlwaysAbsolute,
}

impl std::fmt::Display for FilesystemHostOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}

#[cfg(test)]
mod tests {
    use super::{FilesystemHostOperation, PathResultExposure};
    use std::collections::BTreeSet;

    #[test]
    fn names_and_operation_tags_are_unique_and_round_trip() {
        let mut names = BTreeSet::new();
        let mut tags = BTreeSet::new();
        for operation in FilesystemHostOperation::ALL {
            assert!(names.insert(operation.canonical_name()));
            assert!(tags.insert(operation.operation_tag()));
            assert_eq!(
                FilesystemHostOperation::from_canonical_name(operation.canonical_name()),
                Some(operation)
            );
        }
    }

    #[test]
    fn canonical_trait_surface_matches_the_closed_operation_set() {
        let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../../omega/language/std/filesystem_host.omg");
        let source = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
        let declared: BTreeSet<_> = source
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("machine "))
            .filter_map(|signature| signature.split_once('(').map(|(name, _)| name))
            .collect();
        let encoded: BTreeSet<_> = FilesystemHostOperation::ALL
            .into_iter()
            .map(FilesystemHostOperation::canonical_name)
            .collect();
        assert_eq!(declared, encoded);
    }

    #[test]
    fn path_result_exposure_covers_conditional_and_unconditional_cases() {
        let exposed: Vec<_> = FilesystemHostOperation::ALL
            .into_iter()
            .filter_map(|operation| {
                let exposure = operation.path_result_exposure();
                (exposure != PathResultExposure::None)
                    .then_some((operation.canonical_name(), exposure))
            })
            .collect();
        assert_eq!(
            exposed,
            vec![
                ("read_link", PathResultExposure::MayBeAbsolute),
                ("canonicalize", PathResultExposure::AlwaysAbsolute),
                (
                    "final_path_name_by_handle",
                    PathResultExposure::AlwaysAbsolute
                ),
            ]
        );
    }
}
