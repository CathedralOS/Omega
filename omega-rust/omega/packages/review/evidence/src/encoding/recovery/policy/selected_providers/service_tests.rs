use super::{Error, fixtures, tests::recover};
use omega_effects::provider_plan::{ServiceProgressSubject, ServiceResultClaim};
use psi_language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarrySuspension, DomainPredicateBody,
};

#[test]
fn complete_service_claim_result_carry_and_progress_fields_remain_distinct() {
    let original = fixtures::complete();
    let baseline = original.canonical_bytes().unwrap();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    let method = &mut changed.plans[0].methods[0];
    method.has_result = true;
    method.result_type_identity = Some("u64".into());
    method.signature.result = Some(crate::record::PackageReviewTypeIdentity {
        canonical: "u64".into(),
    });
    method.result_claims.push(ServiceResultClaim {
        domain: "Established".into(),
        effective_carry: fixtures::carry(),
    });
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].entry_claims[0].predicate_body = DomainPredicateBody::Bodyless;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].termination_premises[0].subject =
        ServiceProgressSubject::ProviderReceiver;
    changed.plans[0].methods[0].authority.progress_premises[0].subject =
        ServiceProgressSubject::ProviderReceiver;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].termination_premises[0]
        .subject_projections
        .push("Field".into());
    changed.plans[0].methods[0].authority.progress_premises[0]
        .subject_projections
        .push(fixtures::nominal("Field"));
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].termination_premises[0]
        .establishment_routes
        .pop();
    changed.plans[0].methods[0].authority.progress_premises[0]
        .establishment_routes
        .pop();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].synchronous_invocations.clear();
    changed.plans[0].methods[0]
        .authority
        .synchronous_invocations
        .clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0]
        .service_reach
        .push("TargetService".into());
    changed.plans[0].methods[0]
        .authority
        .service_reach
        .push(fixtures::nominal("TargetService"));
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].terminates_guarantee = false;
    changed.plans[0].methods[0].termination_premises.clear();
    changed.plans[0].methods[0]
        .authority
        .progress_premises
        .clear();
    cases.push(changed);
    for changed in cases {
        let bytes = changed.canonical_bytes().unwrap();
        assert_ne!(bytes, baseline);
        assert_eq!(recover(&bytes).unwrap(), changed);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::SelectedProviders(&changed),
        );
    }
    for bits in 0..16 {
        let mut policy = original.clone();
        let carry = &mut policy.plans[0].methods[0].entry_claims[0].effective_carry;
        carry.suspension = if bits & 1 == 0 {
            CarrySuspension::Forbidden
        } else {
            CarrySuspension::Allowed
        };
        carry.cpu = if bits & 2 == 0 {
            CarryCpu::Origin
        } else {
            CarryCpu::Any
        };
        carry.host_thread = if bits & 4 == 0 {
            CarryHostThread::Origin
        } else {
            CarryHostThread::Any
        };
        carry.address = if bits & 8 == 0 {
            CarryAddress::Stable
        } else {
            CarryAddress::Movable
        };
        let bytes = policy.canonical_bytes().unwrap();
        assert_eq!(recover(&bytes).unwrap(), policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::SelectedProviders(&policy),
        );
    }
}

#[test]
fn service_predicate_carry_authority_and_provider_binding_tags_are_closed() {
    let bytes = fixtures::complete().canonical_bytes().unwrap();
    let domain = b"Authorized";
    let domain_end = bytes
        .windows(domain.len())
        .position(|window| window == domain)
        .unwrap()
        + domain.len();
    for (offset, invalid) in [(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 1)] {
        let mut changed = bytes.clone();
        changed[domain_end + offset] = invalid;
        assert_eq!(recover(&changed), Err(Error::InvalidTag));
    }
    let mut binding = vec![1];
    binding.extend_from_slice(&19i64.to_le_bytes());
    binding.push(0);
    let binding_offset = bytes
        .windows(binding.len())
        .position(|window| window == binding)
        .unwrap();
    let mut changed = bytes;
    changed[binding_offset] = 255;
    assert_eq!(recover(&changed), Err(Error::InvalidTag));
}

#[test]
fn typed_service_signature_changes_without_calling_remain_observable() {
    let original = fixtures::complete();
    assert!(original.plans[0].methods[0].calling.is_none());
    let baseline = original.canonical_bytes().unwrap();
    let mut changed = original.clone();
    let signature = &mut changed.plans[0].methods[0].signature;
    signature
        .schema_arguments
        .push(crate::record::PackageReviewTypeIdentity {
            canonical: "SchemaArgument".into(),
        });
    signature.schema_lifetime_parameter_count = 1;
    signature
        .requirement_arguments
        .push(crate::record::PackageReviewTypeIdentity {
            canonical: "RequirementArgument".into(),
        });
    signature.requirement_lifetime_arguments.push(0);
    signature.requirement_lifetime_parameter_count = 1;
    signature.parameters[0].type_identity.canonical = "OtherSourceQualifiedType".into();
    signature.parameters[0].is_mutable = true;
    signature.static_parameters.clear();
    assert_eq!(
        changed.plans[0].methods[0].parameter_type_identities,
        original.plans[0].methods[0].parameter_type_identities,
        "legacy schema strings remain unchanged"
    );
    let bytes = changed.canonical_bytes().unwrap();
    assert_ne!(
        bytes, baseline,
        "typed source signature cannot be replaced by legacy strings"
    );
    assert_eq!(recover(&bytes).unwrap(), changed);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::SelectedProviders(&changed),
    );
}

#[test]
fn family_authority_coverage_target_and_package_fields_reject_unknown_values() {
    let policy = fixtures::complete();
    let bytes = policy.canonical_bytes().unwrap();
    let target = policy.target.identity();
    let target = target.as_str().as_bytes();
    let family_target = bytes
        .windows(target.len())
        .rposition(|window| window == target)
        .unwrap();
    for offset in [
        family_target + target.len(),
        family_target + target.len() + 1,
    ] {
        let mut changed = bytes.clone();
        changed[offset] = 255;
        assert_eq!(recover(&changed), Err(Error::InvalidTag));
    }
    let mut changed = bytes.clone();
    changed[family_target] = b'?';
    assert_eq!(recover(&changed), Err(Error::InvalidValue));
    let mut changed = bytes;
    let package = crate::encoding::SELECTED_PROVIDER_POLICY_MAGIC.len() + 2;
    changed[package..package + 32].fill(0);
    assert_eq!(recover(&changed), Err(Error::InvalidIdentity));
}
