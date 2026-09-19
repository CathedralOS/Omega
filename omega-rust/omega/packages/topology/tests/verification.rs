//! The independent source-free consumer: `verify_plan` reconstructs the same
//! graph the producer checked, binds every roster record to an admitted
//! component description, replays every selected predicate, and rejects
//! corrupt, stale, forged, or unselected content without loading anything.

mod support;

use std::collections::BTreeSet;

use support::*;
use topology_plan::deployment_plan::predicate;
use topology_plan::*;

#[test]
fn golden_payment_plan_verifies_and_reconstructs_the_same_graph() {
    let (request_bytes, plan_bytes, plan) = payment_pair();
    let checked = verify_plan(&plan_bytes, &request_bytes, &payment_components())
        .expect("golden plan verifies");
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
fn a_record_without_an_admission_rejects() {
    let (request_bytes, plan_bytes, _) = payment_pair();
    // No admissions at all: every Component-role instance is unbound.
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &[]),
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
fn a_component_cannot_pose_as_an_external_participant() {
    // billing's subject has a supplied admission, so relabeling its record
    // `ExternalParticipant` — claiming unverified inventory — rejects.
    let (request_bytes, _, mut plan) = payment_pair();
    plan.instances[2].role = InstanceRole::ExternalParticipant;
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::ComponentBinding {
            instance,
            failure: ComponentBindingFailure::ExternalSubjectVerified,
        }) if instance.as_str() == "billing"
    ));
}

#[test]
fn a_substituted_component_record_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    // The forged closure digest decodes — and now rejects against the
    // admission's own closure instead of verifying under a divergent
    // published subject.
    plan.instances[0].component.completeness = Completeness::VerifiedComplete {
        closure: identity(0x30),
    };
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::ComponentBinding {
            failure: ComponentBindingFailure::Substituted {
                field: "completeness"
            },
            ..
        })
    ));
}

#[test]
fn a_substituted_endpoint_inventory_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    // An extra demanded import is an unaccounted communication path: the
    // verified inventory binds the roster exactly.
    plan.instances[0].endpoints.push(Endpoint {
        slot: 1,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
    plan.instances[0]
        .endpoints
        .sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::ComponentBinding {
            failure: ComponentBindingFailure::Substituted { field: "endpoints" },
            ..
        })
    ));
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
        verify_plan(&plan_bytes, &stale_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
                verify_plan(&corrupt, &request_bytes, &payment_components()),
                Err(PlanRejection::UnverifiedCompleteness { tag: found }) if found == tag
            ),
            "completeness tag {tag} must reject"
        );
    }
}

#[test]
fn a_forged_assumption_rejects_as_substitution() {
    // The plan's assumption roster must equal the description's: a forged
    // entry is a substituted record, not an owner-acceptance question.
    let (request_bytes, _, mut plan) = payment_pair();
    plan.instances[0].component.assumptions = vec![identity(0xAA)];
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::ComponentBinding {
            failure: ComponentBindingFailure::Substituted {
                field: "assumptions"
            },
            ..
        })
    ));
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
fn an_unaccepted_assumption_rejects() {
    // api's real record demands the port-mechanism assumption; drop the
    // owner's acceptance and the honest record still rejects.
    let (mut request, _, components, instances) = assumption_roster();
    request.accepted_assumptions = Vec::new();
    let request_bytes = encode_request(&request).unwrap();
    let (plan, _) = compose_plan(
        &request,
        &request_bytes,
        instances,
        payment_bindings(),
        verifier(),
        &components,
    )
    .expect("composition does not adjudicate owner acceptance");
    let plan_bytes = encode_plan(&plan).unwrap();
    let assumption = component_description::port_mechanism_assumption(
        semantic_vocabulary::ServiceId::new(PORT_SERVICE).unwrap(),
        PORT_NUMBER,
        PORT_VALUE,
    );
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &components),
        Err(PlanRejection::UnacceptedAssumption { assumption: found, .. }) if found == assumption
    ));
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
fn unselected_transport_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.bindings[0].transport = identity(0x78);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::UnselectedPolicyExecutable { .. })
    ));
}

#[test]
fn a_missing_owner_required_policy_rejects() {
    let (request_bytes, _, mut plan) = payment_pair();
    plan.policies.remove(0);
    let plan_bytes = encode_plan(&plan).unwrap();
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::PolicyNotSatisfied { .. })
    ));
}

/// api that demands a second requirement, and the roster built around it.
fn dual_import_roster() -> (Vec<AdmittedComponent>, Vec<PlanInstance>) {
    let api2 = component_module(
        &["AuthorizationBoundary::authorize", "BillingBoundary::post"],
        &[],
    );
    let components = vec![
        admit(&api2),
        admit(&authorization_module()),
        admit(&billing_module()),
    ];
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    (components, instances)
}

#[test]
fn a_forged_satisfied_flag_does_not_survive_replay() {
    // A real bypass: api's second demanded import is bound straight to
    // billing's requirement export, routing around authorization. No honest
    // composition emits this plan — `compose_plan` fails `only_via` — so a
    // forger recomits a plan composed under a weaker request and records
    // the required rows as satisfied.
    let (components, instances) = dual_import_roster();
    let mut weak = payment_request();
    weak.instances[0].subject = subject_of(&components[0]);
    weak.policies = vec![PolicyCall::no_route(
        PolicySelector::new([name("billing")]),
        PolicySelector::new([name("api")]),
    )];
    let weak_bytes = encode_request(&weak).unwrap();
    let bypass_bindings = vec![
        Binding {
            import: endpoint(0, 0),
            export: endpoint(1, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(0, 1),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
    ];
    let (weak_plan, _) = compose_plan(
        &weak,
        &weak_bytes,
        instances,
        bypass_bindings,
        verifier(),
        &components,
    )
    .expect("the weaker request composes");

    // The owner's real request demands the routing policy too.
    let mut request = weak.clone();
    request.policies = payment_policies();
    let request_bytes = encode_request(&request).unwrap();

    // Forge: the required policy set recorded as satisfied, recommitted to
    // the real request.
    let mut forged = weak_plan;
    forged.request_commitment = request_commitment(&request_bytes);
    forged.policies = vec![
        forged.policies[0].clone(),
        ExecutedPolicy {
            call: request.policies[1].clone(),
            verifier: verifier(),
            outcome: PolicyOutcome::Satisfied {
                certificate: Certificate::OnlyVia {
                    path: vec![0, 1, 2],
                    reachable: vec![0],
                },
            },
        },
    ];
    let forged_bytes = encode_plan(&forged).unwrap();
    // The recorded certificate is replayed against the reconstructed graph:
    // the forged "satisfied" verdict cannot survive the outcome comparison —
    // replay recomputes the only_via row as violated before the recorded
    // certificate is even reached.
    assert!(matches!(
        verify_plan(&forged_bytes, &request_bytes, &components),
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
    let api2 = component_module(
        &["AuthorizationBoundary::authorize", "LoggingBoundary::log"],
        &[],
    );
    let logging = component_module(
        &["BillingBoundary::post"],
        &[(
            "LoggingBoundary::log",
            "LoggingProvider",
            "LoggingProvider::log",
        )],
    );
    let components = vec![
        admit(&api2),
        admit(&authorization_module()),
        admit(&billing_module()),
        admit(&logging),
    ];
    let mut request = payment_request();
    request.instances[0].subject = subject_of(&components[0]);
    request.instances.push(RequestedInstance {
        name: name("logging"),
        subject: subject_of(&components[3]),
    });
    // Compose under a request that demands only the disjointness policy:
    // the laundering route is itself honest inventory and satisfies no_route.
    let mut weak = request.clone();
    weak.policies = vec![PolicyCall::no_route(
        PolicySelector::new([name("billing")]),
        PolicySelector::new([name("api")]),
    )];
    let weak_bytes = encode_request(&weak).unwrap();
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
        verified_instance(name("logging"), &components[3]),
    ];
    let bindings = vec![
        Binding {
            import: endpoint(0, 0),
            export: endpoint(1, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(0, 1),
            export: endpoint(3, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(3, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
    ];
    let (weak_plan, _) = compose_plan(
        &weak,
        &weak_bytes,
        instances,
        bindings,
        verifier(),
        &components,
    )
    .expect("composition under the weaker request succeeds");

    let request_bytes = encode_request(&request).unwrap();
    let mut forged = weak_plan;
    forged.request_commitment = request_commitment(&request_bytes);
    forged.policies = vec![
        forged.policies[0].clone(),
        ExecutedPolicy {
            call: request.policies[1].clone(),
            verifier: verifier(),
            outcome: PolicyOutcome::Satisfied {
                certificate: Certificate::OnlyVia {
                    path: vec![0, 1, 2],
                    reachable: vec![0],
                },
            },
        },
    ];
    let forged_bytes = encode_plan(&forged).unwrap();
    assert!(matches!(
        verify_plan(&forged_bytes, &request_bytes, &components),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
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
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::InvalidPolicy { .. })
    ));
}

#[test]
fn corrupt_plan_bytes_reject_as_malformed() {
    let (request_bytes, mut plan_bytes, _) = payment_pair();
    // Truncate inside the policy section.
    plan_bytes.truncate(plan_bytes.len() - 4);
    assert!(matches!(
        verify_plan(&plan_bytes, &request_bytes, &payment_components()),
        Err(PlanRejection::MalformedPlan(_))
    ));
}
