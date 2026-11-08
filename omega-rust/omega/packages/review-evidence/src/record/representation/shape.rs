//! Complete checked shape graph for one representation demand.

use crate::record::accessors::field_projection_accessors;
pub type PackageReviewBoundaryShapeClass = calling_conventions::BoundaryValueClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShape {
    pub(crate) class: PackageReviewBoundaryShapeClass,
    pub(crate) byte_size: u16,
    pub(crate) alignment: u16,
}

impl PackageReviewBoundaryShape {
    field_projection_accessors!(valued class: PackageReviewBoundaryShapeClass, byte_size: u16);
    field_projection_accessors!(valued alignment: u16);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeField {
    pub(crate) shape: u16,
    pub(crate) byte_offset: u16,
}

impl PackageReviewBoundaryShapeField {
    field_projection_accessors!(valued shape: u16, byte_offset: u16);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeGraph {
    pub(crate) shapes: Vec<PackageReviewBoundaryShape>,
    pub(crate) fields: Vec<PackageReviewBoundaryShapeField>,
    pub(crate) parameters: Vec<u16>,
    pub(crate) result: Option<u16>,
}

impl PackageReviewBoundaryShapeGraph {
    field_projection_accessors!(sliced shapes: &[PackageReviewBoundaryShape]);
    field_projection_accessors!(sliced fields: &[PackageReviewBoundaryShapeField]);
    field_projection_accessors!(sliced parameters: &[u16]);
    field_projection_accessors!(copied result: Option<u16>);
}
