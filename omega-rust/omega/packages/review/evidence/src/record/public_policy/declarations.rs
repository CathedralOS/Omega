use crate::record::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTraitShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) is_boundary: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parents: Vec<PackageReviewTraitParent>,
    pub(crate) requirements: Vec<PackagePolicyTraitRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTraitRequirement {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) has_default_realization: bool,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewTraitRequirementParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackagePolicyCrashRoute>,
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) termination: PackagePolicyTermination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyConformanceShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) subject: PackageReviewConformanceSubject,
    pub(crate) interface: PackageReviewEvidenceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDomainShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) target_type: PackageReviewTypeIdentity,
    pub(crate) index_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) predicate_body: language_semantics::DomainPredicateBody,
    pub(crate) predicate_facts: Vec<PackageReviewContractFact>,
    pub(crate) alias_expansion: Option<Vec<PackageReviewDomainAliasAtom>>,
    pub(crate) classification: Option<PackageReviewDomainClassification>,
    pub(crate) semantic_roles: Vec<PackageReviewDomainSemanticRole>,
    pub(crate) establishment_routes: Vec<PackageReviewDomainEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyOperatorShape {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) is_boundary: bool,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewCallableParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackagePolicyCrashRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDataShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) kind: PackageReviewDataKind,
    pub(crate) supply: language_semantics::DataSupplyMode,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) properties: PackageReviewDataProperties,
    pub(crate) zero_gated: bool,
    pub(crate) invariants: Vec<PackageReviewContractFact>,
    pub(crate) retired_identities: Vec<u64>,
    pub(crate) members: Vec<PackageReviewDataMember>,
}
