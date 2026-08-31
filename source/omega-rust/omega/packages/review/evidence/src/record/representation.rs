//! Stable representation trust evidence.

use super::identity::PackageReviewNominalIdentity;

/// The role of one representation row. Producer availability publishes an
/// ordinary public candidate and accepts no consumer selection or ABI.
/// Consumer demand receives a separate role only after complete physical
/// realization evidence exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationTcbKind {
    Unbound,
    ProducerAvailability {
        conformance: PackageReviewNominalIdentity,
        carrier: PackageReviewNominalIdentity,
    },
}

/// Distinct representation-TCB evidence for one opaque boundary datum. An
/// unbound row is owned by the package declaring the opaque; an availability
/// row is owned by the package declaring the public conformance and may target
/// a dependency's opaque. Neither claims that a consumer selected it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewRepresentationTcb {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) kind: PackageReviewRepresentationTcbKind,
}

impl PackageReviewRepresentationTcb {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }

    pub const fn kind(&self) -> &PackageReviewRepresentationTcbKind {
        &self.kind
    }
}
