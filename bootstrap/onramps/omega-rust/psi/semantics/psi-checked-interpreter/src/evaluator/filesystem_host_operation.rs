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

/// Canonical authored operand kind and order for one `FilesystemHost`
/// requirement. This is evaluator ABI schema, not provider behavior: even an
/// operand a modeled provider does not use must be prepared exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemHostOperandKind {
    PathBytes,
    Bytes,
    I32,
    U32,
    I64,
    U64,
    MutableBytes,
    MutableI64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemHostResultKind {
    I32,
    I64,
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

    pub(super) const fn operand_kinds(self) -> &'static [FilesystemHostOperandKind] {
        use FilesystemHostOperandKind as K;
        match self {
            Self::Create => &[K::PathBytes, K::I32],
            Self::Open => &[K::PathBytes, K::I32],
            Self::OpenCreate => &[K::PathBytes, K::I32, K::I32],
            Self::Read => &[K::I32, K::MutableBytes, K::U64],
            Self::Write => &[K::I32, K::Bytes],
            Self::ReadAt => &[K::I32, K::MutableBytes, K::U64, K::I64],
            Self::WriteAt => &[K::I32, K::Bytes, K::I64],
            Self::Close => &[K::I32],
            Self::Remove => &[K::PathBytes],
            Self::Seek => &[K::I32, K::I64, K::I32],
            Self::CreateDir => &[K::PathBytes, K::I32],
            Self::RemoveDir => &[K::PathBytes],
            Self::CreateDirName => &[K::Bytes, K::I32],
            Self::OpenAt => &[K::I32, K::Bytes, K::I32],
            Self::UnlinkAt => &[K::I32, K::Bytes, K::I32],
            Self::SetPermissions => &[K::PathBytes, K::U32],
            Self::SetFilePermissions => &[K::I32, K::U32],
            Self::Rename => &[K::PathBytes, K::PathBytes],
            Self::HardLink => &[K::PathBytes, K::PathBytes],
            Self::Symlink => &[K::PathBytes, K::PathBytes],
            Self::ReadLink => &[K::PathBytes, K::MutableBytes, K::U64],
            Self::Canonicalize => &[K::PathBytes, K::MutableBytes],
            Self::ReadDir => &[K::I32, K::MutableBytes, K::U64, K::MutableI64],
            Self::FindFirst => &[K::Bytes, K::MutableBytes],
            Self::FindNext => &[K::I64, K::MutableBytes],
            Self::FindClose => &[K::I64],
            Self::CreateHardLink => &[K::PathBytes, K::PathBytes, K::I64],
            Self::OpenPathHandle => &[K::PathBytes, K::U32, K::U32, K::I64, K::U32, K::U32, K::I64],
            Self::CloseHandle => &[K::I64],
            Self::GetOsfHandle => &[K::I32],
            Self::FinalPathNameByHandle => &[K::I64, K::MutableBytes, K::U64, K::U32],
            Self::SetFileTime => &[K::I64, K::I64, K::Bytes, K::Bytes],
            Self::LockFileEx => &[K::I64, K::U32, K::U32, K::U32, K::U32, K::MutableBytes],
            Self::UnlockFile => &[K::I64, K::U32, K::U32, K::U32, K::U32],
            Self::GetLastError => &[],
            Self::RemoveName => &[K::Bytes],
            Self::RemoveDirName => &[K::Bytes],
            Self::ReadMetadata => &[K::PathBytes, K::MutableBytes],
            Self::ReadFileMetadata => &[K::I32, K::MutableBytes],
            Self::ReadSymlinkMetadata => &[K::PathBytes, K::MutableBytes],
            Self::SetLen => &[K::I32, K::I64],
            Self::SetFileTimes => &[K::I32, K::MutableBytes],
            Self::Sync => &[K::I32],
            Self::SyncData => &[K::I32],
            Self::Duplicate => &[K::I32],
            Self::LockFile => &[K::I32, K::I32],
            Self::ChangeOwner => &[K::PathBytes, K::I32, K::I32],
            Self::ChangeOwnerNoFollow => &[K::PathBytes, K::I32, K::I32],
            Self::ChangeFileOwner => &[K::I32, K::I32, K::I32],
            Self::Errno => &[],
        }
    }

    pub(super) const fn result_kind(self) -> FilesystemHostResultKind {
        use FilesystemHostResultKind as R;
        match self {
            Self::Read
            | Self::Write
            | Self::ReadAt
            | Self::WriteAt
            | Self::Seek
            | Self::ReadLink
            | Self::Canonicalize
            | Self::ReadDir
            | Self::FindFirst
            | Self::OpenPathHandle
            | Self::GetOsfHandle
            | Self::FinalPathNameByHandle => R::I64,
            Self::Create
            | Self::Open
            | Self::OpenCreate
            | Self::Close
            | Self::Remove
            | Self::CreateDir
            | Self::RemoveDir
            | Self::CreateDirName
            | Self::OpenAt
            | Self::UnlinkAt
            | Self::SetPermissions
            | Self::SetFilePermissions
            | Self::Rename
            | Self::HardLink
            | Self::Symlink
            | Self::FindNext
            | Self::FindClose
            | Self::CreateHardLink
            | Self::CloseHandle
            | Self::SetFileTime
            | Self::LockFileEx
            | Self::UnlockFile
            | Self::GetLastError
            | Self::RemoveName
            | Self::RemoveDirName
            | Self::ReadMetadata
            | Self::ReadFileMetadata
            | Self::ReadSymlinkMetadata
            | Self::SetLen
            | Self::SetFileTimes
            | Self::Sync
            | Self::SyncData
            | Self::Duplicate
            | Self::LockFile
            | Self::ChangeOwner
            | Self::ChangeOwnerNoFollow
            | Self::ChangeFileOwner
            | Self::Errno => R::I32,
        }
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
    use super::{
        FilesystemHostOperandKind, FilesystemHostOperation, FilesystemHostResultKind,
        PathResultExposure,
    };
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
    fn canonical_trait_signatures_match_exact_operand_order_and_kind() {
        use FilesystemHostOperandKind as K;

        let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../../omega/language/std/filesystem_host.omg");
        let source = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
        let signatures: std::collections::BTreeMap<_, _> = source
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("machine "))
            .map(|signature| {
                let (name, tail) = signature.split_once('(').expect("machine argument list");
                let (arguments, result) = tail.split_once(')').expect("closed argument list");
                let kinds = if arguments.trim().is_empty() {
                    Vec::new()
                } else {
                    arguments
                        .split(',')
                        .map(|argument| {
                            let (_, authored_type) =
                                argument.split_once(':').expect("named authored operand");
                            match authored_type.trim() {
                                "&[u8] in Path" => K::PathBytes,
                                "&[u8]" => K::Bytes,
                                "&mut [u8]" => K::MutableBytes,
                                "&mut i64" => K::MutableI64,
                                "i32" => K::I32,
                                "u32" => K::U32,
                                "i64" => K::I64,
                                "u64" => K::U64,
                                other => panic!("unrecognized filesystem operand type `{other}`"),
                            }
                        })
                        .collect()
                };
                let result = match result
                    .trim()
                    .strip_prefix("-> ")
                    .and_then(|result| result.split_whitespace().next())
                    .expect("filesystem result type")
                {
                    "i32" => FilesystemHostResultKind::I32,
                    "i64" => FilesystemHostResultKind::I64,
                    other => panic!("unrecognized filesystem result type `{other}`"),
                };
                (name, (kinds, result))
            })
            .collect();
        assert_eq!(signatures.len(), 50);
        for operation in FilesystemHostOperation::ALL {
            assert_eq!(
                signatures
                    .get(operation.canonical_name())
                    .map(|(operands, result)| (operands.as_slice(), *result)),
                Some((operation.operand_kinds(), operation.result_kind())),
                "operand schema drift for `{operation}`"
            );
        }
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
