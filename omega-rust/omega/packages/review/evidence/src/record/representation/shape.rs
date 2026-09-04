//! Complete checked shape graph for one representation demand.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryShapeClass {
    Integer,
    Float,
    Reference,
    FixedArray { element: u16, length: u16 },
    Record { first_field: u16, field_count: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShape {
    pub(crate) class: PackageReviewBoundaryShapeClass,
    pub(crate) byte_size: u16,
    pub(crate) alignment: u16,
}

impl PackageReviewBoundaryShape {
    pub const fn class(self) -> PackageReviewBoundaryShapeClass {
        self.class
    }

    pub const fn byte_size(self) -> u16 {
        self.byte_size
    }

    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeField {
    pub(crate) shape: u16,
    pub(crate) byte_offset: u16,
}

impl PackageReviewBoundaryShapeField {
    pub const fn shape(self) -> u16 {
        self.shape
    }

    pub const fn byte_offset(self) -> u16 {
        self.byte_offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryShapeGraph {
    pub(crate) shapes: Vec<PackageReviewBoundaryShape>,
    pub(crate) fields: Vec<PackageReviewBoundaryShapeField>,
    pub(crate) parameters: Vec<u16>,
    pub(crate) result: Option<u16>,
}

impl PackageReviewBoundaryShapeGraph {
    pub fn shapes(&self) -> &[PackageReviewBoundaryShape] {
        &self.shapes
    }

    pub fn fields(&self) -> &[PackageReviewBoundaryShapeField] {
        &self.fields
    }

    pub fn parameters(&self) -> &[u16] {
        &self.parameters
    }

    pub const fn result(&self) -> Option<u16> {
        self.result
    }
}
