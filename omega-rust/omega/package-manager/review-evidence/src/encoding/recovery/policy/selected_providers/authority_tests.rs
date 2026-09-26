use super::{Error, fixtures, tests::recover};
use crate::record::{PackageReviewNominalOwner, PackageReviewSynchronousInvocation};

#[test]
fn authority_owners_are_observable_without_changing_legacy_service_names() {
    let original = fixtures::complete();
    let baseline = original.canonical_bytes().unwrap();
    let foreign = PackageReviewNominalOwner::Package(
        semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]).unwrap(),
    );
    let mut cases = Vec::new();
    let mut changed = original.clone();
    let authority = &mut changed.plans[0].methods[0].authority;
    authority.service_reach[1].owner = foreign;
    let PackageReviewSynchronousInvocation::Service(service) =
        &mut authority.synchronous_invocations[0]
    else {
        panic!("fixture service invocation")
    };
    service.owner = foreign;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0]
        .authority
        .synchronous_invocations[0] = PackageReviewSynchronousInvocation::Parameter(0);
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].authority.progress_premises[0]
        .profile
        .owner = foreign;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].authority.progress_premises[0].subject_projections[0].owner =
        foreign;
    cases.push(changed);
    let mut changed = original.clone();
    let route =
        &mut changed.plans[0].methods[0].authority.progress_premises[0].establishment_routes[0];
    route.requirement_owner.owner = foreign;
    route.requirement.owner = foreign;
    cases.push(changed);
    for policy in cases {
        let method = &policy.plans[0].methods[0];
        let prior = &original.plans[0].methods[0];
        assert_eq!(method.service_reach, prior.service_reach);
        assert_eq!(
            method.synchronous_invocations,
            prior.synchronous_invocations
        );
        assert_eq!(method.termination_premises, prior.termination_premises);
        let bytes = policy.canonical_bytes().unwrap();
        assert_ne!(bytes, baseline);
        assert_eq!(recover(&bytes).unwrap(), policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::SelectedProviders(&policy),
        );
    }
}

#[test]
fn typed_authority_order_repetition_and_progress_tags_reject() {
    let original = fixtures::complete();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.plans[0].methods[0]
        .authority
        .service_reach
        .reverse();
    cases.push(changed);
    let mut changed = original.clone();
    let invocations = &mut changed.plans[0].methods[0]
        .authority
        .synchronous_invocations;
    invocations.push(invocations[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].authority.progress_premises[0]
        .establishment_routes
        .reverse();
    cases.push(changed);
    let mut changed = original.clone();
    let premises = &mut changed.plans[0].methods[0].authority.progress_premises;
    premises.push(premises[0].clone());
    cases.push(changed);
    for policy in cases {
        assert!(policy.canonical_bytes().is_err());
    }
    let bytes = original.canonical_bytes().unwrap();
    let projection = b"Context::progress";
    let route_count = bytes
        .windows(projection.len())
        .position(|window| window == projection)
        .unwrap()
        + projection.len();
    let mut changed = bytes;
    changed[route_count + 8] = 2;
    assert_eq!(
        recover(&changed),
        Err(Error::InvalidTag),
        "unknown typed establishment route"
    );
}
