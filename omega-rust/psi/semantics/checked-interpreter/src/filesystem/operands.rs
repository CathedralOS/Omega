//! Scalar, byte, path-like, rooted-path and mutable operands.

use crate::filesystem::FilesystemGrantRootIdentity;

/// Canonical non-handle scalar value consumed by one filesystem operation.
/// Width and signedness remain explicit so ABI-distinct operands never compare
/// equal merely because the interpreter carries both in an `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemScalarOperandValue {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemScalarOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) value: FilesystemScalarOperandValue,
}

impl FilesystemScalarOperand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> FilesystemScalarOperandValue {
        self.value
    }
}

/// Immutable non-path payload bytes consumed by one operation. Rooted paths
/// and path-like byte aliases stay in path evidence so compiler/cache absolute
/// spellings cannot leak through this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemByteOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact bytes consumed where an operation assigns path-like meaning without
/// consuming a rooted path grant. Keeping this distinct from immutable payload
/// bytes and authorized rooted paths preserves the operation's operand roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemPathLikeOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemPathLikeOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One compiler-rooted path at the instant its authored operand successfully
/// resolves during call preparation. This preserves the portable input before
/// physical provider-path lowering. It is not an authorization result: a later
/// grant check may resolve symlinks to a different canonical rooted location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemRootedPathOperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

impl FilesystemRootedPathOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

/// Complete state of one mutable byte carrier at the instant its authored
/// operand successfully resolves. This preparation-prefix row is distinct from
/// the provider-visible pre/post row because evaluating a later argument may
/// alias and mutate the carrier before provider invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemMutableByteOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact value of one mutable i64 carrier at the instant its authored operand
/// successfully resolves. Provider-visible pre/post timing remains separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64OperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) value: i64,
}

impl FilesystemMutableI64OperandResolution {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> i64 {
        self.value
    }
}

/// Complete provider-visible state of one mutable byte carrier immediately
/// before and after the operation's provider invocation. Both vectors equal
/// the resolved carrier capacity; unchanged tails remain explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) pre_bytes: Vec<u8>,
    pub(crate) post_bytes: Vec<u8>,
}

impl FilesystemMutableByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn pre_bytes(&self) -> &[u8] {
        &self.pre_bytes
    }

    pub fn post_bytes(&self) -> &[u8] {
        &self.post_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64Operand {
    pub(crate) operand_ordinal: u8,
    pub(crate) pre_value: i64,
    pub(crate) post_value: i64,
}

impl FilesystemMutableI64Operand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn pre_value(self) -> i64 {
        self.pre_value
    }

    pub const fn post_value(self) -> i64 {
        self.post_value
    }
}
