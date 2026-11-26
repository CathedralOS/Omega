// Stable representation trust evidence for opaque types, boundary shapes, physical movement, and target geometry.

mod movement;
mod policy;
mod target;

use crate::record::accessors::field_projection_accessors;
pub(crate) use movement::{
    PackageReviewBoundaryCallingPolicy, PackageReviewMachineRegister,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement,
};
pub use movement::{
    PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation, PackageReviewSystemVEightbyteClass,
};
pub use policy::PackagePolicyRepresentation;
pub(crate) use policy::{
    PackagePolicyRepresentationAvailability, PackagePolicyRepresentationDemand,
    PackagePolicyRepresentationSelection,
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

// Complete checked shape graph for one representation demand.

pub type PackageReviewBoundaryShapeClass = calling_conventions::BoundaryValueClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShape {
    pub class: PackageReviewBoundaryShapeClass,
    pub byte_size: u16,
    pub alignment: u16,
}

impl PackageReviewBoundaryShape {
    field_projection_accessors!(valued class: PackageReviewBoundaryShapeClass, byte_size: u16);
    field_projection_accessors!(valued alignment: u16);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeField {
    pub shape: u16,
    pub byte_offset: u16,
}

impl PackageReviewBoundaryShapeField {
    field_projection_accessors!(valued shape: u16, byte_offset: u16);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeGraph {
    pub shapes: Vec<PackageReviewBoundaryShape>,
    pub fields: Vec<PackageReviewBoundaryShapeField>,
    pub parameters: Vec<u16>,
    pub result: Option<u16>,
}

impl PackageReviewBoundaryShapeGraph {
    field_projection_accessors!(sliced shapes: &[PackageReviewBoundaryShape]);
    field_projection_accessors!(sliced fields: &[PackageReviewBoundaryShapeField]);
    field_projection_accessors!(sliced parameters: &[u16]);
    field_projection_accessors!(copied result: Option<u16>);
}
