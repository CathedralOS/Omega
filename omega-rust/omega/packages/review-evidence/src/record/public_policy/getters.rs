use crate::record::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyOperatorShape, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
    PackagePolicyTypeParameter, PackageReviewCallableContract, PackageReviewConformanceBound,
    PackageReviewContractFact, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentRoute, PackageReviewDomainSemanticRole,
    PackageReviewEvidenceInterface, PackageReviewNominalIdentity, PackageReviewOperatorCoordinate,
    PackageReviewSignatureParameter, PackageReviewTraitParent, PackageReviewTypeIdentity,
};

impl PackagePolicyTraitShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }
    pub fn parents(&self) -> &[PackageReviewTraitParent] {
        &self.parents
    }
    pub fn requirements(&self) -> &[PackagePolicyTraitRequirement] {
        &self.requirements
    }
}
impl PackagePolicyTraitRequirement {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub const fn spelling(&self) -> Option<language_core::OperatorSpelling> {
        self.spelling
    }
    pub const fn has_default_realization(&self) -> bool {
        self.has_default_realization
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewSignatureParameter] {
        &self.parameters
    }
    crate::record::accessors::package_policy_callable_signature_accessors!();
}
impl PackagePolicyConformanceShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub const fn interface(&self) -> &PackageReviewEvidenceInterface {
        &self.interface
    }
}
impl PackagePolicyDomainShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub const fn target_type(&self) -> &PackageReviewTypeIdentity {
        &self.target_type
    }
    pub fn index_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.index_arguments
    }
    pub const fn predicate_body(&self) -> language_semantics::DomainPredicateBody {
        self.predicate_body
    }
    pub const fn classification(&self) -> Option<PackageReviewDomainClassification> {
        self.classification
    }
    pub fn semantic_roles(&self) -> &[PackageReviewDomainSemanticRole] {
        &self.semantic_roles
    }
    pub fn establishment_routes(&self) -> &[PackageReviewDomainEstablishmentRoute] {
        &self.establishment_routes
    }
}
impl PackagePolicyOperatorShape {
    pub const fn coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.coordinate
    }
    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }
    pub const fn spelling(&self) -> Option<language_core::OperatorSpelling> {
        self.spelling
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewSignatureParameter] {
        &self.parameters
    }
    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
    }
    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }
}
impl PackagePolicyDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
    }
    pub const fn supply(&self) -> language_semantics::DataSupplyMode {
        self.supply
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub const fn properties(&self) -> PackageReviewDataProperties {
        self.properties
    }
    pub const fn zero_gated(&self) -> bool {
        self.zero_gated
    }
    pub fn invariants(&self) -> &[PackageReviewContractFact] {
        &self.invariants
    }
    pub fn retired_identities(&self) -> &[u64] {
        &self.retired_identities
    }
    pub fn members(&self) -> &[PackageReviewDataMember] {
        &self.members
    }
}
