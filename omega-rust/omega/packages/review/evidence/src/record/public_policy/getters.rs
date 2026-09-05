use crate::record::*;

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
    pub const fn spelling(&self) -> Option<psi_language_core::OperatorSpelling> {
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
    pub fn parameters(&self) -> &[PackageReviewTraitRequirementParameter] {
        &self.parameters
    }
    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
    }
    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }
    pub fn published_crash(&self) -> &[PackagePolicyCrashRoute] {
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
    pub const fn termination(&self) -> &PackagePolicyTermination {
        &self.termination
    }
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
    pub const fn subject(&self) -> &PackageReviewConformanceSubject {
        &self.subject
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
    pub const fn predicate_body(&self) -> psi_language_semantics::DomainPredicateBody {
        self.predicate_body
    }
    pub fn predicate_facts(&self) -> &[PackageReviewContractFact] {
        &self.predicate_facts
    }
    pub fn alias_expansion(&self) -> Option<&[PackageReviewDomainAliasAtom]> {
        self.alias_expansion.as_deref()
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
    pub const fn spelling(&self) -> Option<psi_language_core::OperatorSpelling> {
        self.spelling
    }
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }
    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
    }
    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }
    pub fn published_crash(&self) -> &[PackagePolicyCrashRoute] {
        &self.published_crash
    }
}
impl PackagePolicyDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }
    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
    }
    pub const fn supply(&self) -> psi_language_semantics::DataSupplyMode {
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
