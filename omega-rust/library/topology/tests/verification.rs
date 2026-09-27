//! The independent source-free consumer: `verify_plan` reconstructs the same
//! graph the producer checked, binds every roster record to an admitted
//! component description, replays every selected predicate, and rejects
//! corrupt, stale, forged, or unselected content without loading anything.

mod support;

use std::collections::BTreeSet;

use support::*;
use topology_plan::*;

#[test]
fn a_record_without_an_admission_rejects() {
    let (request_bytes, plan_bytes, _) = payment_pair();
    // No admissions at all: every Component-role instance is unbound.
    assert!(matches!(
        verify_plan(
            &plan_bytes,
            &request_bytes,
            &Vec::<AdmittedComponent>::new()
        ),
        Err(PlanRejection::ComponentBinding {
            failure: ComponentBindingFailure::MissingVerifiedComponent,
            ..
        })
    ));
    // A partial admission set leaves the rest unbound — billing's record is
    // honestly formed but nothing verifies its subject.
    let mut partial = payment_components();
    partial.remove(2);
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &partial),
        Err(PlanRejection::ComponentBinding {
            instance,
            failure: ComponentBindingFailure::MissingVerifiedComponent,
        }) if instance.as_str() == "billing"
    ));
}

#[test]
fn forged_completeness_tags_reject_at_decode() {
    let (request_bytes, plan_bytes, _) = payment_pair();
    // The completeness tag byte follows each instance's two 32-byte identities
    // after its length-prefixed name and role byte. Locate the verified tag
    // (0x01) for api: magic+version+commitment+count = 4+4+32+4 = 44, then
    // name len(4)+"api"(3)+role(1)+subject(32)+profile(32) → tag at 44+4+3+1+64.
    let tag_offset = 44 + 4 + 3 + 1 + 64;
    assert_eq!(plan_bytes[tag_offset], 1, "fixture layout sanity");
    for tag in [0u8, 2u8] {
        let mut corrupt = plan_bytes.clone();
        corrupt[tag_offset] = tag;
        assert!(
            matches!(
                verify_plan(&corrupt, &request_bytes, &payment_components()),
                Err(PlanRejection::UnverifiedCompleteness { tag: found }) if found == tag
            ),
            "completeness tag {tag} must reject"
        );
    }
}

/// Compose the roster whose api really does write the port — the
/// assumption-bearing variant of the payment graph.
fn assumption_roster() -> (
    TopologyRequest,
    Vec<u8>,
    Vec<AdmittedComponent>,
    Vec<PlanInstance>,
) {
    let (module, assumption) = assumption_api_module();
    let components = vec![
        admit_with(&module, BTreeSet::from([assumption])),
        admit(&authorization_module()),
        admit(&billing_module()),
    ];
    let mut request = payment_request();
    request.instances[0].subject = subject_of(&components[0]);
    request.accepted_assumptions = vec![assumption];
    let request_bytes = encode_request(&request).unwrap();
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    (request, request_bytes, components, instances)
}

#[test]
fn an_owner_accepted_assumption_passes() {
    let (request, request_bytes, components, instances) = assumption_roster();
    let (plan, _) = compose_plan(
        &request,
        &request_bytes,
        instances,
        payment_bindings(),
        verifier(),
        &components,
    )
    .expect("accepted assumption admits");
    let plan_bytes = encode_plan(&plan).unwrap();
    verify_plan(&plan_bytes, &request_bytes, &components).expect("accepted assumption verifies");
}

#[test]
fn invalid_policy_selectors_reject() {
    // A request requiring an overlapping-selector policy and a plan carrying
    // it: the row is required, and replay rejects its selectors.
    let mut request = payment_request();
    request.policies = vec![PolicyCall::only_via(
        PolicySelector::new([name("api")]),
        PolicySelector::new([name("api")]),
        PolicySelector::new([name("authorization")]),
    )];
    let request_bytes = encode_request(&request).unwrap();
    let (_, _, mut plan) = payment_pair();
    plan.policies = vec![ExecutedPolicy {
        call: request.policies[0].clone(),
        verifier: verifier(),
        outcome: PolicyOutcome::Satisfied {
            certificate: Certificate::OnlyVia {
                path: vec![0],
                reachable: vec![0],
            },
        },
    }];
    plan.request_commitment = request_commitment(&request_bytes);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::InvalidPolicy { .. })
    ));
}
