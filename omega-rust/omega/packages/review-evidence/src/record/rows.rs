use super::identity::PackageReviewToolchainSourceIdentity;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationRole {
    Suspension,
    Blocking,
    ServiceReach,
    /// Exact authored operator application that demanded one reviewed
    /// role-specific realization.
    BoundaryApplicationUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationOwner {
    Package(PackageKeyIdentity),
    Toolchain(PackageReviewToolchainSourceIdentity),
}

/// Compiler-validated package-relative source coordinate used only to explain
/// a canonical review row. Absolute resolver/cache paths never enter it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewSourceLocation {
    pub(crate) owner: PackageReviewSourceLocationOwner,
    pub(crate) relative_path: String,
    pub(crate) start_byte: u64,
    pub(crate) end_byte: u64,
    pub(crate) role: PackageReviewSourceLocationRole,
}
