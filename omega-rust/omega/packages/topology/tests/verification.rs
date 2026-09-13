//! The independent source-free consumer: `verify_plan` reconstructs the same
//! graph the producer checked, replays every selected predicate, and rejects
//! corrupt, stale, forged, or unselected content without loading anything.

mod support;

use support::*;
use topology_plan::deployment_plan::predicate;
use topology_plan::*;

#[test]
fn golden_payment_plan_verifies_and_reconstructs_the_same_graph() {
    let (request_bytes, plan_bytes, plan) = payment_pair();
    let checked = verify_plan(&plan_bytes, &request_bytes).expect("golden plan verifies");
    assert_eq!(checked.subject, plan_subject(&plan_bytes));
    // The reconstructed graph is the producer's normalized graph, not the
    // plan's assertion of it.
    assert_eq!(checked.graph.instances(), plan.instances.as_slice());
    assert_eq!(checked.graph.bindings(), plan.bindings.as_slice());
    // `composition checked` only — this value carries no installation claim.
    let names: Vec<&str> = checked
        .graph
        .instances()
        .iter()
        .map(|instance| instance.name.as_str())
        .collect();
    assert_eq!(names, ["api", "authorization", "billing"]);
}

#[test]
fn a_stale_request_rejects_even_when_well_formed() {
    let (_, plan_bytes, _) = payment_pair();
    // A correctly formed request for a different roster (renamed billing) is
    // still not the request this plan was composed for.
    let mut stale = payment_request();
    stale.instances[2].name = name("receivables");
    let stale_bytes = encode_request(&stale).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &stale_bytes),
        Err(PlanRejection::StaleRequest { .. })
    ));
}

#[test]
fn swapped_component_code_rejects_as_roster_mismatch() {
    let (request_bytes, plan_bytes, mut plan) = payment_pair();
    plan.instances[2].component.subject = identity(0x34);
    let plan_bytes = {
        let _ = plan_bytes;
        encode_plan(&plan).unwrap()
    };
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::RosterMismatch { .. })
    ));
}

#[test]
fn an_undeclared_extra_instance_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    // "auditor" sorts between "api" and "authorization" — canonical position.
    plan.instances
        .insert(1, instance("auditor", 0x44, &[9], &[]));
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::RosterMismatch { .. })
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
                verify_plan(&corrupt, &request_bytes),
                Err(PlanRejection::UnverifiedCompleteness { tag: found }) if found == tag
            ),
            "completeness tag {tag} must reject"
        );
    }
}

#[test]
fn unaccepted_assumptions_reject() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.instances[0].component.assumptions = vec![identity(0xAA)];
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::UnacceptedAssumption { .. })
    ));
}

#[test]
fn an_owner_accepted_assumption_passes() {
    let mut request = payment_request();
    let (_, _, mut plan) = payment_pair();
    request.accepted_assumptions = vec![identity(0xAA)];
    let request_bytes = encode_request(&request).unwrap();
    plan.instances[0].component.assumptions = vec![identity(0xAA)];
    plan.request_commitment = request_commitment(&request_bytes);
    let plan_bytes = encode_plan(&plan).unwrap();
    verify_plan(&plan_bytes, &request_bytes).expect("accepted assumption admits");
}

#[test]
fn unselected_transport_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.bindings[0].transport = identity(0x78);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::UnselectedTransport { .. })
    ));
}

#[test]
fn unselected_policy_executable_rejects_without_loading() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.policies[0].verifier = identity(0x51);
    let plan_bytes = encode_plan(&plan).unwrap();
    // Rejection is by identity comparison — nothing was loaded or executed.
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::UnselectedPolicyExecutable { .. })
    ));
}

#[test]
fn a_missing_owner_required_policy_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.policies.remove(0);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::MissingRequiredPolicy { .. })
    ));
}

#[test]
fn an_extra_unselected_policy_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    // no_route sorts before only_via — canonical position is the front.
    plan.policies.insert(
        0,
        ExecutedPolicy {
            call: PolicyCall::no_route(
                PolicySelector::new([name("api")]),
                PolicySelector::new([name("authorization")]),
            ),
            verifier: verifier(),
            outcome: PolicyOutcome::Violated {
                violation: Violation::Bypass { path: vec![0, 1] },
            },
        },
    );
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::UnexpectedPolicy { .. })
    ));
}

#[test]
fn a_recorded_violation_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.policies[1].outcome = PolicyOutcome::Violated {
        violation: Violation::Bypass {
            path: vec![0, 1, 2],
        },
    };
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::PolicyNotSatisfied { .. })
    ));
}

#[test]
fn a_forged_satisfied_flag_does_not_survive_replay() {
    let (request_bytes, _, mut plan) = payment_pair();
    // Add a direct api -> billing bypass binding, then forge the certificate
    // as if the composition had succeeded without it.
    plan.instances[0].endpoints.push(Endpoint {
        slot: 2,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
    // Import key (0,2) sorts between (0,1) and (1,1) — canonical position.
    plan.bindings.insert(
        1,
        Binding {
            import: endpoint(0, 2),
            export: endpoint(2, 1),
            transport: transport(),
        },
    );
    let plan_bytes = encode_plan(&plan).unwrap();
    // The recorded certificate is replayed against the reconstructed graph:
    // the forged "satisfied" verdict cannot survive the outcome comparison —
    // replay recomputes the only_via row as violated before the recorded
    // certificate is even reached.
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::ReplayMismatch { index: 1 })
    ));
}

#[test]
fn an_indirect_bypass_through_a_roster_member_rejects_on_replay() {
    // The bypass runs api -> logging -> billing through a legitimate roster
    // member: every demanded import is bound, the roster and transports match
    // the request exactly, and the recorded outcomes are honestly formed — so
    // normalization and every comparison succeed, and only independent replay
    // of the selected predicates sees the route around authorization.
    let mut request = payment_request();
    request.instances.push(RequestedInstance {
        name: name("logging"),
        subject: identity(0x44),
    });
    let request_bytes = encode_request(&request).unwrap();

    // logging holds a billing channel legitimately and exports its own
    // service on slot 1; api has no route to it, so this four-instance
    // composition satisfies only_via honestly.
    let mut instances = payment_instances();
    instances.push(instance("logging", 0x44, &[1], &[1]));
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(3, 1),
        export: endpoint(2, 1),
        transport: transport(),
    });
    let (plan, _) = compose_plan(&request, &request_bytes, instances, bindings, verifier())
        .expect("composition without the api -> logging edge succeeds");

    // Forge: declare api's second import and the laundering binding while
    // keeping the outcomes recorded for the bypass-free graph.
    let mut forged = plan;
    forged.instances[0].endpoints.push(Endpoint {
        slot: 2,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
    // Binding (0,2)->(3,1) sorts between (0,1)->(1,1) and (1,1)->(2,1).
    forged.bindings.insert(
        1,
        Binding {
            import: endpoint(0, 2),
            export: endpoint(3, 1),
            transport: transport(),
        },
    );
    let forged_bytes = encode_plan(&forged).unwrap();
    assert!(matches!(
        verify_plan(&forged_bytes, &request_bytes),
        Err(PlanRejection::ReplayMismatch { index: 1 })
    ));

    // The checked witness is the real two-hop route: replay on the
    // reconstructed graph reports api -> logging -> billing, not a generic
    // failure.
    let graph = NormalizedGraph::new(forged.instances.clone(), forged.bindings.clone()).unwrap();
    let call = PolicyCall::only_via(
        PolicySelector::new([name("api")]),
        PolicySelector::new([name("billing")]),
        PolicySelector::new([name("authorization")]),
    );
    let PolicyEvaluation::Decided(PolicyOutcome::Violated {
        violation: Violation::Bypass { path },
    }) = evaluate_policy(&graph, &call)
    else {
        panic!("replay must surface the bypass")
    };
    let names: Vec<&str> = path
        .iter()
        .map(|&index| graph.instances()[index as usize].name.as_str())
        .collect();
    assert_eq!(names, ["api", "logging", "billing"]);

    // And the recorded certificate has no repair: the honest reachable set is
    // no longer closed under the actual edges, while the set that *is* closed
    // under them contains the target — closure and disjointness cannot both
    // hold, so no reachable-set forgery survives structural checking.
    let recorded = &forged.policies[1].outcome;
    assert!(matches!(
        predicate::check_outcome(&graph, &call, recorded),
        Err(CertificateRejection::NotClosed { .. })
    ));
    let closed_set = PolicyOutcome::Satisfied {
        certificate: Certificate::OnlyVia {
            path: vec![0, 1, 2],
            reachable: vec![0, 2, 3],
        },
    };
    assert!(matches!(
        predicate::check_outcome(&graph, &call, &closed_set),
        Err(CertificateRejection::NotDisjoint { vertex: 2 })
    ));
}

#[test]
fn a_corrupt_certificate_rejects_on_structure() {
    let (request_bytes, _, mut plan) = payment_pair();
    // api is index 0; replacing the certificate's reachable set with one that
    // omits a source vertex fails the closure check even though the recorded
    // verdict happens to be right.
    let only_via_row = plan
        .policies
        .iter_mut()
        .find(|row| row.call.predicate == PolicyPredicate::OnlyVia)
        .unwrap();
    only_via_row.outcome = PolicyOutcome::Satisfied {
        certificate: Certificate::OnlyVia {
            path: vec![0, 1, 2],
            reachable: vec![1, 2],
        },
    };
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::InvalidCertificate { .. })
    ));
}

#[test]
fn a_modified_binding_after_check_rejects_independently() {
    let (request_bytes, _, mut plan) = payment_pair();
    // Rewire authorization's import to a nonexistent exporter instance slot —
    // normalization rejects the reconstructed graph before any verdict.
    plan.bindings[1].export = endpoint(2, 9);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::InvalidGraph(_))
    ));
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
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::InvalidPolicy { .. })
    ));
}

#[test]
fn corrupt_plan_bytes_reject_as_malformed() {
    let (request_bytes, mut plan_bytes, _) = payment_pair();
    // Truncate inside the policy section.
    plan_bytes.truncate(plan_bytes.len() - 4);
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes),
        Err(PlanRejection::MalformedPlan(_))
    ));
}
