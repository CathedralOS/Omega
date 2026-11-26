// Public API policy shape declarations for traits, requirements, conformances, domains, operators, and data.

use crate::record::PackageReviewServiceIdentity;
use crate::record::{
    PackagePolicyCrashRoute, PackagePolicyTermination, PackagePolicyTypeParameter,
    PackageReviewCallableContract, PackageReviewConformanceBound, PackageReviewContractFact,
    PackageReviewDataKind, PackageReviewDataMember, PackageReviewDataProperties,
    PackageReviewDomainClassification, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewEvidenceInterface, PackageReviewNominalIdentity,
    PackageReviewOperatorCoordinate, PackageReviewParameterQualification,
    PackageReviewSignatureParameter, PackageReviewSynchronousInvocation, PackageReviewTraitParent,
    PackageReviewTypeIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTraitShape {
    pub identity: PackageReviewNominalIdentity,
    pub is_boundary: bool,
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub parents: Vec<PackageReviewTraitParent>,
    pub requirements: Vec<PackagePolicyTraitRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTraitRequirement {
    pub identity: PackageReviewNominalIdentity,
    pub spelling: Option<language_core::OperatorSpelling>,
    pub has_default_realization: bool,
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub conformance_bounds: Vec<crate::record::PackageReviewConformanceBound>,
    pub parameters: Vec<PackageReviewSignatureParameter>,
    /// Entry i names the source formal for structural parameter i. Version 37 omitted this list.
    pub structural_parameter_positions: Option<Vec<u32>>,
    pub return_type: Option<PackageReviewTypeIdentity>,
    pub contracts: Vec<PackageReviewCallableContract>,
    /// None is open; Some(empty) is an explicit empty ceiling.
    pub published_crash: Option<Vec<PackagePolicyCrashRoute>>,
    pub service_reach: Option<Vec<PackageReviewServiceIdentity>>,
    pub service_reach_is_installation_bound: bool,
    pub synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub admitted_same_stack_bytes: u64,
    pub termination: PackagePolicyTermination,
    pub returned_borrow_origins: Vec<crate::record::PackageReviewReturnedBorrowOrigin>,
    pub parameter_qualification_preconditions: Vec<PackageReviewParameterQualification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyConformanceShape {
    pub identity: PackageReviewNominalIdentity,
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub interface: PackageReviewEvidenceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDomainShape {
    pub identity: PackageReviewNominalIdentity,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub target_type: PackageReviewTypeIdentity,
    pub index_arguments: Vec<PackageReviewTypeIdentity>,
    pub predicate_presence: language_core::DomainPredicatePresence,
    pub predicate_facts: Vec<PackageReviewContractFact>,
    pub alias_expansion: Option<Vec<PackageReviewNominalIdentity>>,
    pub classification: Option<PackageReviewDomainClassification>,
    pub semantic_roles: Vec<PackageReviewDomainSemanticRole>,
    pub establishment_routes: Vec<PackageReviewDomainEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyOperatorShape {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) is_boundary: bool,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackagePolicyCrashRoute>,
}

/// The alias owns a public name and binders. Its expansion is the existing carrier's checked type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTypeAlias {
    pub identity: PackageReviewNominalIdentity,
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub expansion: PackageReviewTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDataShape {
    pub identity: PackageReviewNominalIdentity,
    pub kind: PackageReviewDataKind,
    pub supply: language_core::DataSupplyMode,
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub properties: PackageReviewDataProperties,
    pub zero_gated: bool,
    pub invariants: Vec<PackageReviewContractFact>,
    pub retired_identities: Vec<u64>,
    pub members: Vec<PackageReviewDataMember>,
}
