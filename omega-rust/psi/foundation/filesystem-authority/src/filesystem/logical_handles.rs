//! Logical handle identities, kinds, inputs and outputs.

/// Evaluator-issued identity for one filesystem descriptor or handle lifetime.
///
/// The identity is allocated independently of provider token values. Reusing a
/// runtime descriptor after close therefore produces a fresh identity, while
/// every operation during one live lifetime refers to the same identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemLogicalHandleIdentity(u64);

impl FilesystemLogicalHandleIdentity {
    #[doc(hidden)]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Closed resource domain for a logical filesystem handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleKind {
    /// POSIX/CRT-style integer file descriptor.
    Descriptor,
    /// Native path/file handle, distinct from a CRT descriptor on Windows.
    Native,
    /// Directory-enumeration cursor returned by `find_first`.
    Find,
}

/// Resolution of one authored handle operand against earlier successful
/// operations in the same evaluator run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleInputResolution {
    Resolved(FilesystemLogicalHandleIdentity),
    /// The canonical ABI explicitly permits a null handle in this position.
    Null,
    /// No live logical lifetime owns the supplied provider token.
    Unknown,
}

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemLogicalHandleInput {
        pub(crate) operand_ordinal: u8,
        pub(crate) kind: FilesystemLogicalHandleKind,
        pub(crate) resolution: FilesystemLogicalHandleInputResolution,
    }
);

impl FilesystemLogicalHandleInput {
    pub const fn is_unknown_native(self) -> bool {
        matches!(
            (self.kind, self.resolution),
            (
                FilesystemLogicalHandleKind::Native,
                FilesystemLogicalHandleInputResolution::Unknown
            )
        )
    }
}

/// Provenance for a successfully returned logical handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleOutputSource {
    /// A new independently owned resource was opened.
    Created,
    /// A new descriptor lifetime was duplicated from an existing descriptor.
    Duplicated(FilesystemLogicalHandleIdentity),
    /// A native handle view was borrowed from an existing descriptor.
    Borrowed(FilesystemLogicalHandleIdentity),
}

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemLogicalHandleOutput {
        pub(crate) kind: FilesystemLogicalHandleKind,
        pub(crate) identity: FilesystemLogicalHandleIdentity,
        pub(crate) source: FilesystemLogicalHandleOutputSource,
    }
);
