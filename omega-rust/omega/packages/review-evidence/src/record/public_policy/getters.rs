use crate::record::accessors::field_projection_accessors;
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
    field_projection_accessors!(copied is_boundary: bool);
    field_projection_accessors!(sliced parents: &[PackageReviewTraitParent]);
    field_projection_accessors!(sliced requirements: &[PackagePolicyTraitRequirement]);
}
impl PackagePolicyTraitRequirement {
    crate::record::accessors::record_accessors!(
        identity,
        spelling,
        lifetime_parameter_count,
        type_parameters,
        parameters
    );
    field_projection_accessors!(copied has_default_realization: bool);

    crate::record::accessors::package_policy_callable_signature_accessors!();
}
impl PackagePolicyConformanceShape {
    crate::record::accessors::record_accessors!(
        identity,
        lifetime_parameter_count,
        type_parameters
    );
    field_projection_accessors!(borrowed interface: &PackageReviewEvidenceInterface);
}
impl PackagePolicyDomainShape {
    crate::record::accessors::record_accessors!(identity, type_parameters);
    field_projection_accessors!(borrowed target_type: &PackageReviewTypeIdentity);
    field_projection_accessors!(sliced index_arguments: &[PackageReviewTypeIdentity]);
    field_projection_accessors!(copied predicate_body: language_semantics::DomainPredicateBody);
    field_projection_accessors!(copied classification: Option<PackageReviewDomainClassification>);
    field_projection_accessors!(sliced semantic_roles: &[PackageReviewDomainSemanticRole]);
    field_projection_accessors!(sliced establishment_routes: &[PackageReviewDomainEstablishmentRoute]);
}
impl PackagePolicyOperatorShape {
    crate::record::accessors::record_accessors!(
        spelling,
        lifetime_parameter_count,
        type_parameters,
        parameters,
        return_type
    );
    field_projection_accessors!(borrowed coordinate: &PackageReviewOperatorCoordinate);
    field_projection_accessors!(copied is_boundary: bool);
    field_projection_accessors!(sliced contracts: &[PackageReviewCallableContract]);
}
impl PackagePolicyDataShape {
    crate::record::accessors::record_accessors!(
        identity,
        lifetime_parameter_count,
        type_parameters
    );
    field_projection_accessors!(borrowed kind: &PackageReviewDataKind);
    field_projection_accessors!(copied supply: language_semantics::DataSupplyMode);
    field_projection_accessors!(copied properties: PackageReviewDataProperties);
    field_projection_accessors!(copied zero_gated: bool);
    field_projection_accessors!(sliced invariants: &[PackageReviewContractFact]);
    field_projection_accessors!(sliced retired_identities: &[u64]);
    field_projection_accessors!(sliced members: &[PackageReviewDataMember]);
}
