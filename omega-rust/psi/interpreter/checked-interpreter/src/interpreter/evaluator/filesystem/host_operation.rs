//! Stable filesystem observation vocabulary for Build I/O and explicitly
//! configured evaluator models. This is not an Omega boundary interface:
//! ordinary library schemas are checked and selected through their own code.
//! Transcript tags belong to the observation protocol and survive library
//! service splits and declaration reordering.

/// Exact modeled operand order. Even an operand the selected observation
/// provider does not use must be prepared exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::interpreter::evaluator) enum FilesystemHostOperandKind {
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
pub(in crate::interpreter::evaluator) enum FilesystemHostResultKind {
    I32,
    I64,
}

// One typed inventory owns tags, operation names and model signatures. Keep
// existing transcript coordinates stable when adding an operation.
macro_rules! filesystem_operations {
    ($( $variant:ident = $tag:literal => $name:literal ($($operand:ident),*) -> $result:ident; )*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u16)]
        pub(crate) enum FilesystemHostOperation { $( $variant = $tag, )* }

        impl FilesystemHostOperation {
            pub(in crate::interpreter::evaluator) fn from_canonical_name(name: &str) -> Option<Self> {
                Some(match name { $( $name => Self::$variant, )* _ => return None })
            }
            pub(in crate::interpreter::evaluator) const fn operation_tag(self) -> u16 { self as u16 }
            pub(in crate::interpreter::evaluator) const fn operand_kinds(self) -> &'static [FilesystemHostOperandKind] {
                match self { $( Self::$variant => &[ $( FilesystemHostOperandKind::$operand, )* ], )* }
            }
            pub(in crate::interpreter::evaluator) const fn result_kind(self) -> FilesystemHostResultKind {
                match self { $( Self::$variant => FilesystemHostResultKind::$result, )* }
            }
            pub(in crate::interpreter::evaluator) const fn canonical_name(self) -> &'static str {
                match self { $( Self::$variant => $name, )* }
            }
        }
    };
}

filesystem_operations! {
    Create = 1 => "create" (PathBytes, I32) -> I32;
    Open = 2 => "open" (PathBytes, I32) -> I32;
    OpenCreate = 3 => "open_create" (PathBytes, I32, I32) -> I32;
    Read = 4 => "read" (I32, MutableBytes, U64) -> I64;
    Write = 5 => "write" (I32, Bytes) -> I64;
    ReadAt = 6 => "read_at" (I32, MutableBytes, U64, I64) -> I64;
    WriteAt = 7 => "write_at" (I32, Bytes, I64) -> I64;
    Close = 8 => "close" (I32) -> I32;
    Remove = 9 => "remove" (PathBytes) -> I32;
    Seek = 10 => "seek" (I32, I64, I32) -> I64;
    CreateDir = 11 => "create_dir" (PathBytes, I32) -> I32;
    RemoveDir = 12 => "remove_dir" (PathBytes) -> I32;
    CreateDirName = 13 => "create_dir_name" (Bytes, I32) -> I32;
    OpenAt = 14 => "open_at" (I32, Bytes, I32) -> I32;
    UnlinkAt = 15 => "unlink_at" (I32, Bytes, I32) -> I32;
    SetPermissions = 16 => "set_permissions" (PathBytes, U32) -> I32;
    SetFilePermissions = 17 => "set_file_permissions" (I32, U32) -> I32;
    Rename = 18 => "rename" (PathBytes, PathBytes) -> I32;
    HardLink = 19 => "hard_link" (PathBytes, PathBytes) -> I32;
    Symlink = 20 => "symlink" (PathBytes, PathBytes) -> I32;
    ReadLink = 21 => "read_link" (PathBytes, MutableBytes, U64) -> I64;
    Canonicalize = 22 => "canonicalize" (PathBytes, MutableBytes) -> I64;
    ReadDir = 23 => "read_dir" (I32, MutableBytes, U64, MutableI64) -> I64;
    FindFirst = 24 => "find_first" (Bytes, MutableBytes) -> I64;
    FindNext = 25 => "find_next" (I64, MutableBytes) -> I32;
    FindClose = 26 => "find_close" (I64) -> I32;
    CreateHardLink = 27 => "create_hard_link" (PathBytes, PathBytes, I64) -> I32;
    OpenPathHandle = 28 => "open_path_handle" (PathBytes, U32, U32, I64, U32, U32, I64) -> I64;
    CloseHandle = 29 => "close_handle" (I64) -> I32;
    GetOsfHandle = 30 => "get_osfhandle" (I32) -> I64;
    FinalPathNameByHandle = 31 => "final_path_name_by_handle" (I64, MutableBytes, U64, U32) -> I64;
    SetFileTime = 32 => "set_file_time" (I64, I64, Bytes, Bytes) -> I32;
    LockFileEx = 33 => "lock_file_ex" (I64, U32, U32, U32, U32, MutableBytes) -> I32;
    UnlockFile = 34 => "unlock_file" (I64, U32, U32, U32, U32) -> I32;
    GetLastError = 35 => "get_last_error" () -> I32;
    RemoveName = 36 => "remove_name" (Bytes) -> I32;
    RemoveDirName = 37 => "remove_dir_name" (Bytes) -> I32;
    ReadMetadata = 38 => "read_metadata" (PathBytes, MutableBytes) -> I32;
    ReadFileMetadata = 39 => "read_file_metadata" (I32, MutableBytes) -> I32;
    ReadSymlinkMetadata = 40 => "read_symlink_metadata" (PathBytes, MutableBytes) -> I32;
    SetLen = 41 => "set_len" (I32, I64) -> I32;
    SetFileTimes = 42 => "set_file_times" (I32, MutableBytes) -> I32;
    Sync = 43 => "sync" (I32) -> I32;
    SyncData = 44 => "sync_data" (I32) -> I32;
    Duplicate = 45 => "duplicate" (I32) -> I32;
    LockFile = 46 => "lock_file" (I32, I32) -> I32;
    ChangeOwner = 47 => "change_owner" (PathBytes, I32, I32) -> I32;
    ChangeOwnerNoFollow = 48 => "change_owner_no_follow" (PathBytes, I32, I32) -> I32;
    ChangeFileOwner = 49 => "change_file_owner" (I32, I32, I32) -> I32;
    Errno = 50 => "errno" () -> I32;
}

impl std::fmt::Display for FilesystemHostOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}
