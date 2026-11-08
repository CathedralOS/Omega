// Public trait shapes, requirements, returned-borrow origins, parameter domain qualifications, and complete conformance interfaces.

use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewConformanceBound;
use crate::record::PackageReviewCrashRoute;
use crate::record::PackageReviewEvidenceInterface;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewSignatureParameter;
use crate::record::PackageReviewSynchronousInvocation;
use crate::record::PackageReviewTermination;
use crate::record::PackageReviewTypeIdentity;
use crate::record::PackageReviewTypeParameter;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewTraitRequirement {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) has_default_realization: bool, // Public conformance behavior; the body itself remains checked source.
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<crate::record::PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>, // Abstract published crash ceiling for this requirement.
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) termination: PackageReviewTermination,
    pub(crate) returned_borrow_origins: Vec<PackageReviewReturnedBorrowOrigin>, // Returned-borrow origins copied from public dynamic slots.
    pub(crate) parameter_qualification_preconditions: Vec<PackageReviewParameterQualification>, // Structural-parameter domain preconditions copied from dynamic slots.
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewTraitShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) is_boundary: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parents: Vec<PackageReviewTraitParent>,
    pub(crate) requirements: Vec<PackageReviewTraitRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewConformanceShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) interface: PackageReviewEvidenceInterface,
}
