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
    /// Body presence is public conformance behavior. The body itself remains
    /// checked source, not a compiler-private IR blob in package evidence.
    pub(crate) has_default_realization: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    /// Abstract published crash ceiling for this requirement. Trait
    /// requirements have no checked body sites or calls of their own.
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>,
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) termination: PackageReviewTermination,
    /// Returned-borrow origins copied from the requirement's public dynamic
    /// slots. Empty when this compilation forms no selector slot.
    pub(crate) returned_borrow_origins: Vec<PackageReviewReturnedBorrowOrigin>,
    /// Structural-parameter domain preconditions copied from those slots.
    pub(crate) parameter_qualification_preconditions: Vec<PackageReviewParameterQualification>,
}

/// One returned reference's public input origin. The index is the slot's
/// dense structural formal, not an authored scalar position.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewReturnedBorrowOrigin {
    pub(crate) origin: PackageReviewBorrowOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBorrowOrigin {
    Parameter { index: u32 },
    Invocation { carrier: PackageReviewTypeIdentity },
}

/// One domain a structural formal must already carry. `structural_index` is
/// the slot's dense structural lane.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewParameterQualification {
    pub(crate) structural_index: u32,
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

/// One independently nameable, public, complete conformance declaration.
///
/// `interface` is the complete normalized inherited requirement map proven by
/// checked lowering. Realization machine names, bodies, and physical code
/// identity are deliberately absent: they are private implementation, not a
/// receiver-nameable package contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewConformanceShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) interface: PackageReviewEvidenceInterface,
}
