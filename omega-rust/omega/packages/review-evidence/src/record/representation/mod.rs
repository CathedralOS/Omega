//! Stable representation trust evidence.

mod movement;
mod policy;
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
pub use policy::{
    PackagePolicyRepresentation, PackagePolicyRepresentationAvailability,
    PackagePolicyRepresentationDemand, PackagePolicyRepresentationSelection,
};
pub use shape::{
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph,
};
pub use target::{
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
};

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
