/*
We define the portable filesystem authority, canonical metadata, operation
observation, and staging sponsorship contracts for Psi evaluation. Our types
record host operation attempts (`FilesystemHostOperation`), required build
output obligations (`BuildOutputObligation`, `BuildOutputReceipt`), canonical
source-tree metadata (`CanonicalFilesystemMetadataIndex`), path grants
(`FsGrants`, `FilesystemGrantRoot`), and append-and-seal staging accounts
(`FilesystemSponsor`).
*/

/*
Because build evaluation and package review must verify that a build program
only touches granted paths and seals every declared output before publication,
we separate authority and observation evidence from the runtime providers that
execute host calls. Keeping stable transcript tags, canonical slash-separated
relative paths, and linear output-obligation states in a target-neutral
foundation crate lets the evaluator, package reviewer, and build orchestrator
agree on the exact evidence format without coupling Psi to host filesystem
executors.
*/

/*
The per-target open-flag bit positions (`open_flags`) mirror the checked target
encoders in `std/targets/<target>/filesystem_impl.omg`, while canonical metadata
rows (`CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION`) and operation attempt
records (`FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION`) carry explicit schema
versions so persisted package-review transcripts reject stale or malformed
evidence fail-closed.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization::codec_bytes;

mod filesystem;
mod filesystem_sponsor;
mod source_capture;

pub use filesystem::*;
pub use filesystem_sponsor::*;
pub use source_capture::*;

pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 20;

pub mod open_flags {
    #[cfg(target_os = "windows")]
    pub const O_CREAT_BIT: i32 = 8;
    #[cfg(target_os = "windows")]
    pub const O_EXCL_BIT: i32 = 10;
    #[cfg(target_os = "windows")]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(target_os = "windows")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(target_os = "macos")]
    pub const O_CREAT_BIT: i32 = 9;
    #[cfg(target_os = "macos")]
    pub const O_EXCL_BIT: i32 = 11;
    #[cfg(target_os = "macos")]
    pub const O_TRUNC_BIT: i32 = 10;
    #[cfg(target_os = "macos")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_CREAT_BIT: i32 = 6;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_EXCL_BIT: i32 = 7;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_APPEND_BIT: i32 = 10;

    pub const fn o_creat(flags: i32) -> bool {
        (flags >> O_CREAT_BIT) & 1 != 0
    }
    pub const fn o_excl(flags: i32) -> bool {
        (flags >> O_EXCL_BIT) & 1 != 0
    }
    pub const fn o_trunc(flags: i32) -> bool {
        (flags >> O_TRUNC_BIT) & 1 != 0
    }
    pub const fn o_append(flags: i32) -> bool {
        (flags >> O_APPEND_BIT) & 1 != 0
    }

    pub const fn wants_write(flags: i32) -> bool {
        let access = flags & 0x3;
        access == 1 || access == 2 || o_creat(flags) || o_trunc(flags) || o_append(flags)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemHostOperandKind {
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
pub enum FilesystemHostResultKind {
    I32,
    I64,
}

macro_rules! filesystem_operations {
    ($( $variant:ident = $tag:literal => $name:literal ($($operand:ident),*) -> $result:ident; )*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u16)]
        pub enum FilesystemHostOperation { $( $variant = $tag, )* }

        impl FilesystemHostOperation {
            pub fn from_canonical_name(name: &str) -> Option<Self> {
                Some(match name { $( $name => Self::$variant, )* _ => return None })
            }
            pub const fn operation_tag(self) -> u16 { self as u16 }
            pub const fn operand_kinds(self) -> &'static [FilesystemHostOperandKind] {
                match self { $( Self::$variant => &[ $( FilesystemHostOperandKind::$operand, )* ], )* }
            }
            pub const fn result_kind(self) -> FilesystemHostResultKind {
                match self { $( Self::$variant => FilesystemHostResultKind::$result, )* }
            }
            pub const fn canonical_name(self) -> &'static str {
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

pub const MAX_BUILD_OUTPUT_OBLIGATIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOutputObligationState {
    Pending,
    Completed {
        completed_at: usize,
        receipt: usize,
    },
    Failed {
        failed_at: usize,
        diagnostic: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutputObligation {
    pub root: FilesystemGrantRootIdentity, // Grant-root identity of the issuing `builder.output` facet.
    pub relative_path: Vec<u8>,            // Declared canonical relative output name.
    pub issued_at: usize, // Count of completed filesystem attempts when `require` issued the obligation.
    pub state: BuildOutputObligationState,
    pub post_completion_mutations: Vec<usize>, // Completed attempt ordinals that mutated this path after completion.
}

impl BuildOutputObligation {
    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn issued_at(&self) -> usize {
        self.issued_at
    }

    pub const fn state(&self) -> &BuildOutputObligationState {
        &self.state
    }

    pub fn post_completion_mutations(&self) -> &[usize] {
        &self.post_completion_mutations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutputReceipt {
    pub obligation: usize, // Index into the run's `build_output_obligations` completed by this receipt.
    pub root: FilesystemGrantRootIdentity,
    pub relative_path: Vec<u8>,
    pub sealed_at: usize, // Filesystem-attempt ordinal that retired the file's last writer.
    pub completed_at: usize, // Count of completed filesystem attempts when `complete` issued the receipt.
}

impl BuildOutputReceipt {
    pub const fn obligation(&self) -> usize {
        self.obligation
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn sealed_at(&self) -> usize {
        self.sealed_at
    }

    pub const fn completed_at(&self) -> usize {
        self.completed_at
    }
}
