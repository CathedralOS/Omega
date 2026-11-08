// Compiler-validated package-relative source coordinates used only to explain a
// canonical review row; absolute resolver or cache paths never enter these
// records.

use super::identity::PackageReviewToolchainSourceIdentity;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PackageReviewSourceLocationRole {
    Suspension,
    Blocking,
    ServiceReach,
    BoundaryApplicationUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PackageReviewSourceLocationOwner {
    Package(PackageKeyIdentity),
    Toolchain(PackageReviewToolchainSourceIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PackageReviewSourceLocation {
    pub(crate) owner: PackageReviewSourceLocationOwner,
    pub(crate) relative_path: String,
    pub(crate) start_byte: u64,
    pub(crate) end_byte: u64,
    pub(crate) role: PackageReviewSourceLocationRole,
}
