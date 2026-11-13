// Public trait shapes, requirements, returned-borrow origins, parameter domain qualifications, and complete conformance interfaces.

use crate::record::PackageReviewNominalIdentity;

use crate::record::PackageReviewTypeIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageReviewTraitCompositionKind {
    Policy,
    ServiceReach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitParent {
    pub(crate) kind: PackageReviewTraitCompositionKind,
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewTraitParent {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewReturnedBorrowOrigin {
    pub(crate) origin: PackageReviewBorrowOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBorrowOrigin {
    Parameter { index: u32 },
    Invocation { carrier: PackageReviewTypeIdentity },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewParameterQualification {
    pub(crate) structural_index: u32, // Dense structural formal lane index.
    pub(crate) domain: PackageReviewNominalIdentity,
}

impl PackageReviewReturnedBorrowOrigin {
    pub fn parameter_index(&self) -> Option<u32> {
        match self.origin {
            PackageReviewBorrowOrigin::Parameter { index } => Some(index),
            PackageReviewBorrowOrigin::Invocation { .. } => None,
        }
    }

    pub fn invocation_carrier(&self) -> Option<&PackageReviewTypeIdentity> {
        match &self.origin {
            PackageReviewBorrowOrigin::Invocation { carrier } => Some(carrier),
            PackageReviewBorrowOrigin::Parameter { .. } => None,
        }
    }
}

impl PackageReviewParameterQualification {
    pub const fn structural_index(&self) -> u32 {
        self.structural_index
    }

    pub const fn domain(&self) -> &PackageReviewNominalIdentity {
        &self.domain
    }
}
