//! Compiler-rooted output coordinates retained for sealed-file custody.

use super::FilesystemGrantRootIdentity;

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
