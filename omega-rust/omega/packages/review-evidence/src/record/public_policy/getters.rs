use crate::record::accessors::field_projection_accessors;
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
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced conformance_bounds: &[PackageReviewConformanceBound]);
    field_projection_accessors!(copied is_boundary: bool);
    field_projection_accessors!(sliced parents: &[PackageReviewTraitParent]);
    field_projection_accessors!(sliced requirements: &[PackagePolicyTraitRequirement]);
}
impl PackagePolicyTraitRequirement {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied spelling: Option<language_core::OperatorSpelling>);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced parameters: &[PackageReviewSignatureParameter]);
    field_projection_accessors!(copied has_default_realization: bool);

    crate::record::accessors::package_policy_callable_signature_accessors!();
}
impl PackagePolicyConformanceShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed interface: &PackageReviewEvidenceInterface);
}
impl PackagePolicyDomainShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed target_type: &PackageReviewTypeIdentity);
    field_projection_accessors!(sliced index_arguments: &[PackageReviewTypeIdentity]);
    field_projection_accessors!(copied predicate_body: language_semantics::DomainPredicateBody);
    field_projection_accessors!(copied classification: Option<PackageReviewDomainClassification>);
    field_projection_accessors!(sliced semantic_roles: &[PackageReviewDomainSemanticRole]);
    field_projection_accessors!(sliced establishment_routes: &[PackageReviewDomainEstablishmentRoute]);
}
impl PackagePolicyOperatorShape {
    field_projection_accessors!(copied spelling: Option<language_core::OperatorSpelling>);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced parameters: &[PackageReviewSignatureParameter]);
    field_projection_accessors!(optional return_type: Option<&PackageReviewTypeIdentity>);
    field_projection_accessors!(borrowed coordinate: &PackageReviewOperatorCoordinate);
    field_projection_accessors!(copied is_boundary: bool);
    field_projection_accessors!(sliced contracts: &[PackageReviewCallableContract]);
}
impl PackagePolicyDataShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed kind: &PackageReviewDataKind);
    field_projection_accessors!(copied supply: language_semantics::DataSupplyMode);
    field_projection_accessors!(copied properties: PackageReviewDataProperties);
    field_projection_accessors!(copied zero_gated: bool);
    field_projection_accessors!(sliced invariants: &[PackageReviewContractFact]);
    field_projection_accessors!(sliced retired_identities: &[u64]);
    field_projection_accessors!(sliced members: &[PackageReviewDataMember]);
}
