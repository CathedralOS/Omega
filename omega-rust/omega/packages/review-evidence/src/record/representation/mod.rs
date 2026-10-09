//! Stable representation trust evidence.

mod movement;
mod policy;
mod shape;
mod target;

pub use movement::{
};
pub(crate) use movement::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationMovementRole,
    PackageReviewOpaqueRepresentationOccurrence, PackageReviewOpaqueRepresentationPathElement,
    PackageReviewSystemVEightbyteClass};
pub use policy::{
    PackagePolicyRepresentation,
};
pub(crate) use policy::{ PackagePolicyRepresentationAvailability,
    PackagePolicyRepresentationDemand, PackagePolicyRepresentationSelection};
pub use shape::{
};
pub(crate) use shape::{
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph};
pub use target::{
};
pub(crate) use target::{
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile};

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
