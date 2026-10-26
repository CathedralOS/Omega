use crate::record::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyOperatorShape, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewDataKind,
    PackageReviewDataMember, PackageReviewDataProperties, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentRoute, PackageReviewDomainSemanticRole,
    PackageReviewEvidenceInterface, PackageReviewOperatorCoordinate, PackageReviewTraitParent,
    PackageReviewTypeIdentity,
};

impl PackagePolicyTraitShape {
    crate::record::accessors::record_accessors!(
        identity,
        lifetime_parameter_count,
        type_parameters,
        conformance_bounds
    );
    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }

    pub fn parents(&self) -> &[PackageReviewTraitParent] {
        &self.parents
    }
    pub fn requirements(&self) -> &[PackagePolicyTraitRequirement] {
        &self.requirements
    }
}
impl PackagePolicyTraitRequirement {
    crate::record::accessors::record_accessors!(
        identity,
        spelling,
        lifetime_parameter_count,
        type_parameters,
        parameters
    );
    pub const fn has_default_realization(&self) -> bool {
        self.has_default_realization
    }

    crate::record::accessors::package_policy_callable_signature_accessors!();
}
impl PackagePolicyConformanceShape {
    crate::record::accessors::record_accessors!(
        identity,
        lifetime_parameter_count,
        type_parameters
    );
    pub const fn interface(&self) -> &PackageReviewEvidenceInterface {
        &self.interface
    }
}
impl PackagePolicyDomainShape {
    crate::record::accessors::record_accessors!(identity, type_parameters);
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
    crate::record::accessors::record_accessors!(
        spelling,
        lifetime_parameter_count,
        type_parameters,
        parameters,
        return_type
    );
    pub const fn coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.coordinate
    }
    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }
}
impl PackagePolicyDataShape {
    crate::record::accessors::record_accessors!(
        identity,
        lifetime_parameter_count,
        type_parameters
    );
    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
    }
    pub const fn supply(&self) -> language_semantics::DataSupplyMode {
        self.supply
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
