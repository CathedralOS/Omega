use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewTraitCompositionKind {
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

impl PackageReviewTraitParent {
    pub const fn kind(&self) -> PackageReviewTraitCompositionKind {
        self.kind
    }

    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn lifetime_arguments(&self) -> &[u32] {
        &self.lifetime_arguments
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitRequirement {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) spelling: Option<psi_language_core::OperatorSpelling>,
    /// Body presence is public conformance behavior. The body itself remains
    /// checked source, not a compiler-private IR blob in package evidence.
    pub(crate) has_default_realization: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewTraitRequirementParameter>,
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

impl PackageReviewTraitRequirement {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn spelling(&self) -> Option<psi_language_core::OperatorSpelling> {
        self.spelling
    }

    pub const fn has_default_realization(&self) -> bool {
        self.has_default_realization
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn parameters(&self) -> &[PackageReviewTraitRequirementParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn published_crash(&self) -> &[PackageReviewCrashRoute] {
        &self.published_crash
    }

    pub fn service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.service_reach
    }

    pub const fn service_reach_is_installation_bound(&self) -> bool {
        self.service_reach_is_installation_bound
    }

    pub fn synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.synchronous_invocations
    }

    pub const fn suspends(&self) -> bool {
        self.suspends
    }

    pub const fn blocks(&self) -> bool {
        self.blocks
    }

    pub const fn termination(&self) -> &PackageReviewTermination {
        &self.termination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) is_boundary: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parents: Vec<PackageReviewTraitParent>,
    pub(crate) requirements: Vec<PackageReviewTraitRequirement>,
}

/// The carrier named by one public complete conformance. A generic carrier is
/// represented by its alpha-normalized conformance-telescope ordinal; a
/// concrete carrier keeps its exact package/toolchain nominal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewConformanceSubject {
    Subjectless,
    TypeParameter(u32),
    Nominal(PackageReviewNominalIdentity),
}

/// One independently nameable, public, complete conformance declaration.
///
/// `interface` is the complete normalized inherited requirement map proven by
/// checked lowering. Realization machine names, bodies, and physical code
/// identity are deliberately absent: they are private implementation, not a
/// receiver-nameable package contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewConformanceShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) subject: PackageReviewConformanceSubject,
    pub(crate) interface: PackageReviewEvidenceInterface,
}

impl PackageReviewConformanceShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn subject(&self) -> &PackageReviewConformanceSubject {
        &self.subject
    }

    pub const fn interface(&self) -> &PackageReviewEvidenceInterface {
        &self.interface
    }
}

impl PackageReviewTraitShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parents(&self) -> &[PackageReviewTraitParent] {
        &self.parents
    }

    pub fn requirements(&self) -> &[PackageReviewTraitRequirement] {
        &self.requirements
    }
}
