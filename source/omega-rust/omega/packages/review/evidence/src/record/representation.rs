//! Stable representation trust evidence.

use super::identity::PackageReviewNominalIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewOpaqueRepresentationApplicationOrigin {
    NamedConformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewOpaqueRepresentationLifecycleDisposition {
    Inert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewOpaqueRepresentationCopyDisposition {
    CheckedSemanticCopy,
}

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
    /// Compiler-rechecked property receipt owned by the selecting package for
    /// one `[copy]` opaque declaration, which may be dependency-owned. This is
    /// target-independent semantic custody, not a runtime by-value demand or
    /// native movement claim.
    SelectedCopyReceipt {
        conformance: PackageReviewNominalIdentity,
        carrier: PackageReviewNominalIdentity,
        representation_schema_version: u16,
        origin: PackageReviewOpaqueRepresentationApplicationOrigin,
        lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
        copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
        conformance_application_commitment: [u8; 32],
        selected_application_commitment: [u8; 32],
    },
}

/// Distinct representation-TCB evidence for one opaque boundary datum. An
/// unbound row is owned by the package declaring the opaque; an availability
/// row is owned by the package declaring the public conformance and may target
/// a dependency's opaque. A selected copy receipt is owned by the selecting
/// package. Only that final role claims a selection, and none claims D26
/// consumer demand.
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
