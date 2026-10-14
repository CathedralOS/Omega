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
