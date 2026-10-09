//! Exact inputs retained by acceptance, not a serialized package review.

use super::PackageLockError;
use package_compilation::AcceptedSemanticBinding;

/// The compiler and explicit semantic bindings accepted for one occurrence.
/// Source, purpose, and target are supplied by the containing occurrence;
/// ordinary compilation must verify those coordinates before using this data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedPackageCompilation {
    compiler_identity: [u8; 32],
    semantic_bindings: Vec<AcceptedSemanticBinding>,
}

impl AcceptedPackageCompilation {
    pub fn new(
        compiler_identity: [u8; 32],
        semantic_bindings: Vec<AcceptedSemanticBinding>,
    ) -> Result<Self, PackageLockError> {
        if compiler_identity == [0; 32] {
            return Err(PackageLockError::CompilerIdentity);
        }
        Ok(Self {
            compiler_identity,
            semantic_bindings,
        })
    }

    pub const fn compiler_identity(&self) -> [u8; 32] {
        self.compiler_identity
    }

    pub fn semantic_bindings(&self) -> &[AcceptedSemanticBinding] {
        &self.semantic_bindings
    }
}
