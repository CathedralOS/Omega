//! Stable representation trust evidence.

mod movement;
mod shape;
mod target;

pub use movement::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationMovementRole,
    PackageReviewOpaqueRepresentationOccurrence, PackageReviewOpaqueRepresentationPathElement,
    PackageReviewSystemVEightbyteClass,
};
pub use shape::{
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph,
};
pub use target::{
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
};

use super::identity::PackageReviewNominalIdentity;
use super::signatures::PackageReviewTypeIdentity;

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
    PlacementOnly,
}

/// The role of one representation row. Producer availability publishes an
/// ordinary public candidate and accepts no consumer selection or ABI.
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
    /// One actual by-value use, owned by the package that selected the
    /// representation. This row retains the complete checked shape graph and
    /// exact replay-validated target movement; an unused selection never
    /// creates it.
    ConsumerDemand {
        boundary_trait: PackageReviewNominalIdentity,
        boundary_arguments: Vec<PackageReviewTypeIdentity>,
        requirement: PackageReviewNominalIdentity,
        requirement_identity: String,
        target: PackageReviewRepresentationTarget,
        conformance: PackageReviewNominalIdentity,
        carrier: PackageReviewNominalIdentity,
        representation_schema_version: u16,
        origin: PackageReviewOpaqueRepresentationApplicationOrigin,
        lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
        copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
        shape_graph: PackageReviewBoundaryShapeGraph,
        occurrences: Vec<PackageReviewOpaqueRepresentationOccurrence>,
        calling_policy: PackageReviewBoundaryCallingPolicy,
        conformance_application_commitment: [u8; 32],
        selected_application_commitment: [u8; 32],
        boundary_plan_commitment: [u8; 32],
    },
}

/// Distinct representation-TCB evidence for one opaque boundary datum. An
/// unbound row is owned by the package declaring the opaque; an availability
/// row by the package declaring the public conformance; and selected receipts
/// and consumer demands by the package that made the selection.
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
