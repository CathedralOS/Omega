//! Producer-side composition coverage: the payment graph succeeds; direct and
//! indirect bypasses, cycles, disconnection, duplicates, incomplete bindings,
//! unbound records, and invalid policy inputs all reject or violate with
//! checked witnesses — never silently.
//!
//! Instance inventory comes from real verified components: reshaping a
//! roster entry means building a different module and admitting it, never
//! editing a `PlanInstance`'s endpoint list by hand.

mod support;

use support::*;
use topology_plan::*;

fn compose(
    request: &TopologyRequest,
    instances: Vec<PlanInstance>,
    bindings: Vec<Binding>,
    components: &[AdmittedComponent],
) -> Result<(DeploymentPlan, Vec<PolicyOutcome>), CompositionError> {
    let request_bytes = encode_request(request).unwrap();
    compose_plan(
        request,
        &request_bytes,
        instances,
        bindings,
        verifier(),
        components,
    )
}

/// The request roster naming the supplied subjects under the given names,
/// in canonical name order — the honest owner intent for a component set.
fn roster_request(
    names: &[&str],
    subjects: &[Identity],
    policies: Vec<PolicyCall>,
) -> TopologyRequest {
    TopologyRequest {
        instances: names
            .iter()
            .zip(subjects.iter())
            .map(|(text, subject)| RequestedInstance {
                name: name(text),
                subject: *subject,
            })
            .collect(),
        policies,
        verifier: verifier(),
        transports: vec![transport()],
        accepted_assumptions: vec![],
    }
}

#[test]
fn the_payment_graph_composes() {
    let request = payment_request();
    let components = payment_components();
    let (plan, outcomes) = compose(
        &request,
        payment_instances(),
        payment_bindings(),
        &components,
    )
    .expect("payment composition succeeds");
    assert_eq!(plan.instances.len(), 3);
    assert_eq!(plan.bindings.len(), 2);
    assert_eq!(plan.policies.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|outcome| matches!(outcome, PolicyOutcome::Satisfied { .. }))
    );
    // Byte-exact canonical artifacts.
    let plan_bytes = encode_plan(&plan).unwrap();
    assert_eq!(decode_plan(&plan_bytes).unwrap(), plan);
}

#[test]
fn a_direct_bypass_rejects_with_the_witness_path() {
    // api demands a second requirement and it is bound straight to billing:
    // the only_via routing obligation is honestly violated.
    let api2 = component_module(
        &["AuthorizationBoundary::authorize", "BillingBoundary::post"],
        &[],
    );
    let components = vec![
        admit(&api2),
        admit(&authorization_module()),
        admit(&billing_module()),
    ];
    let request = roster_request(
        &["api", "authorization", "billing"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(0, 1),
        export: endpoint(2, 1),
        transport: transport(),
    });
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings, &components)
    else {
        panic!("direct bypass must violate")
    };
    let (call, violation) = &evaluations[0];
    assert!(call.contains("only_via"), "{call}");
    let Violation::Bypass { path } = violation else {
        panic!("bypass violation")
    };
    // The witness is the shortest bypass: api -> billing.
    assert_eq!(path, &[0, 2]);
}

#[test]
fn an_indirect_bypass_through_a_helper_rejects() {
    // api -> logging -> billing routes around authorization through a helper
    // service; every participant's imports are still bound.
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
    let request = roster_request(
        &["api", "authorization", "billing", "logging"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
        verified_instance(name("logging"), &components[3]),
    ];
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(0, 1),
        export: endpoint(3, 1),
        transport: transport(),
    });
    bindings.push(Binding {
        import: endpoint(3, 0),
        export: endpoint(2, 1),
        transport: transport(),
    });
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings, &components)
    else {
        panic!("indirect bypass must violate")
    };
    let (_, violation) = &evaluations[0];
    let Violation::Bypass { path } = violation else {
        panic!("bypass violation")
    };
    let names: Vec<&str> = ["api", "logging", "billing"].to_vec();
    let got: Vec<&str> = path
        .iter()
        .map(|&index| match index {
            0 => "api",
            3 => "logging",
            2 => "billing",
            _ => "?",
        })
        .collect();
    assert_eq!(got, names);
}

#[test]
fn cycles_still_terminate_and_check() {
    // authorization also demands an api surface, so its second import binds
    // back to api's canonical entry export — closing a cycle without
    // touching the policies: billing still reaches nothing, and api ->
    // billing still passes through authorization.
    let authorization2 = component_module(
        &["ApiBoundary::entry", "BillingBoundary::post"],
        &[(
            "AuthorizationBoundary::authorize",
            "AuthorizationProvider",
            "AuthorizationProvider::authorize",
        )],
    );
    let components = vec![
        admit(&api_module()),
        admit(&authorization2),
        admit(&billing_module()),
    ];
    let request = roster_request(
        &["api", "authorization", "billing"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    let bindings = vec![
        Binding {
            import: endpoint(0, 0),
            export: endpoint(1, 1),
            transport: transport(),
        },
        Binding {
            // authorization's "BillingBoundary::post" demand (import slot 1:
            // requirements sort by identity) -> billing's requirement export.
            import: endpoint(1, 1),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            // authorization's "ApiBoundary::entry" demand (import slot 0)
            // -> api's canonical entry export, closing the cycle.
            import: endpoint(1, 0),
            export: endpoint(0, 0),
            transport: transport(),
        },
    ];
    compose(&request, instances, bindings, &components).expect("cyclic graph composes");
}

#[test]
fn an_unbound_demanded_import_rejects_before_policies() {
    // api's second demand is never bound: normalization refuses before any
    // policy runs.
    let api2 = component_module(
        &["AuthorizationBoundary::authorize", "LoggingBoundary::log"],
        &[],
    );
    let components = vec![
        admit(&api2),
        admit(&authorization_module()),
        admit(&billing_module()),
    ];
    let request = roster_request(
        &["api", "authorization", "billing"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    let Err(CompositionError::Rejected(CompositionFailure::InvalidGraph(
        GraphError::UnboundImport { .. },
    ))) = compose(&request, instances, payment_bindings(), &components)
    else {
        panic!("unbound import must reject")
    };
}

#[test]
fn a_record_the_admissions_do_not_establish_rejects() {
    // The producer cannot invent inventory either: a hand-authored endpoint
    // on a roster record fails the binding before the graph stage.
    let request = payment_request();
    let components = payment_components();
    let mut instances = payment_instances();
    instances[0].endpoints.push(Endpoint {
        slot: 1,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
    instances[0]
        .endpoints
        .sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
    assert!(matches!(
        compose(&request, instances, payment_bindings(), &components),
        Err(CompositionError::Rejected(
            CompositionFailure::ComponentBinding {
                failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                ..
            }
        ))
    ));
    // And a roster member no admission verifies cannot compose at all.
    let mut instances = payment_instances();
    instances[0] = instance("api", 0x44, &[0], &[]);
    // The request names api's real subject, so the forged subject still
    // fails the roster join first; an unadmitted *correct-subject* record
    // meets MissingVerifiedComponent only when its subject matches nothing
    // supplied — covered for the consumer in verification.rs; here the
    // forged record diverges on every field at once.
    assert!(matches!(
        compose(&request, instances, payment_bindings(), &components),
        Err(CompositionError::Rejected(
            CompositionFailure::RosterMismatch { .. }
        ))
    ));
}

#[test]
fn duplicate_bindings_reject() {
    let request = payment_request();
    let components = payment_components();
    let mut bindings = payment_bindings();
    bindings.push(bindings[0].clone());
    let Err(CompositionError::Rejected(CompositionFailure::InvalidGraph(
        GraphError::DuplicateBinding { .. },
    ))) = compose(&request, payment_instances(), bindings, &components)
    else {
        panic!("duplicate binding must reject")
    };
}

#[test]
fn duplicate_instances_reject() {
    // Normalization itself rejects a duplicated instance name.
    let mut instances = payment_instances();
    instances.insert(1, instances[0].clone()); // adjacent duplicate "api"
    assert!(matches!(
        NormalizedGraph::new(instances, payment_bindings()),
        Err(GraphError::DuplicateInstance { .. })
    ));
    // Through composition the same input also fails the exact roster check.
    let request = payment_request();
    let components = payment_components();
    let mut instances = payment_instances();
    instances.push(instances[0].clone());
    assert!(matches!(
        compose(&request, instances, payment_bindings(), &components),
        Err(CompositionError::Rejected(
            CompositionFailure::RosterMismatch { .. }
        ))
    ));
}

#[test]
fn compatible_endpoints_with_no_connection_gain_no_edge() {
    // billing's requirement export is bound to authorization; its canonical
    // entry export stays unbound — compatible inventory alone creates no
    // edge.
    let request = payment_request();
    let components = payment_components();
    let (plan, _) = compose(
        &request,
        payment_instances(),
        payment_bindings(),
        &components,
    )
    .unwrap();
    let graph = NormalizedGraph::new(plan.instances.clone(), plan.bindings.clone()).unwrap();
    assert!(
        graph.successors(2).is_empty(),
        "billing has no outgoing edge"
    );
    assert_eq!(graph.successors(0), &[1]);
}

#[test]
fn two_instances_of_one_component_stay_distinct() {
    // A second api deployment sharing the component code must not merge with
    // the first: instance names, not subjects, key the graph.
    let components = payment_components();
    let request = roster_request(
        &["api", "api-east", "authorization", "billing"],
        &[
            subject_of(&components[0]),
            subject_of(&components[0]),
            subject_of(&components[1]),
            subject_of(&components[2]),
        ],
        vec![PolicyCall::no_route(
            PolicySelector::new([name("api-east")]),
            PolicySelector::new([name("billing")]),
        )],
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("api-east"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    let bindings = vec![
        Binding {
            import: endpoint(0, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(2, 0),
            export: endpoint(3, 1),
            transport: transport(),
        },
    ];
    // api-east -> authorization -> billing: no_route({api-east},{billing})
    // must still fail — instance identity, not code, carries the policy.
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings, &components)
    else {
        panic!("api-east route must violate no_route")
    };
    assert!(evaluations[0].0.contains("no_route"));
}

#[test]
fn a_self_connection_is_allowed_and_visible() {
    // scheduler demands and realizes its own tick requirement: the binding
    // from its import back to its own export is a real self-edge.
    let scheduler = component_module(
        &["SchedulerBoundary::tick"],
        &[(
            "SchedulerBoundary::tick",
            "SchedulerProvider",
            "SchedulerProvider::tick",
        )],
    );
    let mut components = payment_components();
    components.push(admit(&scheduler));
    let request = roster_request(
        &["api", "authorization", "billing", "scheduler"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
        verified_instance(name("scheduler"), &components[3]),
    ];
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(3, 0),
        export: endpoint(3, 1),
        transport: transport(),
    });
    compose(&request, instances, bindings, &components).expect("self-connection composes");
}

#[test]
fn an_external_participant_composes_without_an_admission() {
    // An explicitly modeled external: it names a subject no admission
    // verifies and contributes only owner-asserted inventory.
    let components = payment_components();
    let mut request = payment_request();
    request.instances.push(RequestedInstance {
        name: name("clock"),
        subject: identity(0x44),
    });
    let mut external = instance("clock", 0x44, &[], &[0]);
    external.role = InstanceRole::ExternalParticipant;
    let mut instances = payment_instances();
    instances.push(external);
    compose(&request, instances, payment_bindings(), &components)
        .expect("an unverified external composes");
}

#[test]
fn swapped_component_code_fails_the_roster() {
    let request = payment_request();
    let components = payment_components();
    let mut instances = payment_instances();
    instances[2].component.subject = identity(0x34);
    assert!(matches!(
        compose(&request, instances, payment_bindings(), &components),
        Err(CompositionError::Rejected(
            CompositionFailure::RosterMismatch { .. }
        ))
    ));
}

#[test]
fn an_unselected_verifier_fails_before_evaluation() {
    let request = payment_request();
    let request_bytes = encode_request(&request).unwrap();
    assert!(matches!(
        compose_plan(
            &request,
            &request_bytes,
            payment_instances(),
            payment_bindings(),
            identity(0x51),
            &payment_components(),
        ),
        Err(CompositionError::Rejected(
            CompositionFailure::UnselectedVerifier
        ))
    ));
}

#[test]
fn invalid_policy_inputs_reject_instead_of_vacuously_satisfying() {
    for call in [
        // empty source set
        PolicyCall::no_route(PolicySelector::new([]), PolicySelector::new([name("api")])),
        // nonexistent member
        PolicyCall::no_route(
            PolicySelector::new([name("ghost")]),
            PolicySelector::new([name("api")]),
        ),
        // overlapping source/target
        PolicyCall::no_route(
            PolicySelector::new([name("api")]),
            PolicySelector::new([name("api")]),
        ),
        // overlapping via set
        PolicyCall::only_via(
            PolicySelector::new([name("api")]),
            PolicySelector::new([name("billing")]),
            PolicySelector::new([name("api")]),
        ),
    ] {
        let mut request = payment_request();
        request.policies = vec![call];
        let request_bytes = encode_request(&request).unwrap();
        let result = compose_plan(
            &request,
            &request_bytes,
            payment_instances(),
            payment_bindings(),
            verifier(),
            &payment_components(),
        );
        assert!(
            matches!(result, Err(CompositionError::PoliciesViolated { .. })),
            "invalid selector must reject"
        );
    }
}

#[test]
fn multiple_edges_between_the_same_pair_keep_all_evidence() {
    // api demands two requirements and authorization realizes both: two
    // distinct demanded imports bind to two distinct offered exports, the
    // adjacency deduplicates for traversal, and both binding rows stay.
    let api2 = component_module(
        &["AuthorizationBoundary::authorize", "QuotaBoundary::check"],
        &[],
    );
    let authorization2 = component_module(
        &["BillingBoundary::post"],
        &[
            (
                "AuthorizationBoundary::authorize",
                "AuthorizationProvider",
                "AuthorizationProvider::authorize",
            ),
            (
                "QuotaBoundary::check",
                "QuotaProvider",
                "QuotaProvider::check",
            ),
        ],
    );
    let components = vec![
        admit(&api2),
        admit(&authorization2),
        admit(&billing_module()),
    ];
    let request = roster_request(
        &["api", "authorization", "billing"],
        &subjects_of(&components),
        payment_policies(),
    );
    let instances = vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ];
    // authorization's exports sort canonical, then by requirement identity:
    // "AuthorizationBoundary::authorize" < "QuotaBoundary::check" -> slots
    // 1 and 2.
    let bindings = vec![
        Binding {
            import: endpoint(0, 0),
            export: endpoint(1, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(0, 1),
            export: endpoint(1, 2),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 0),
            export: endpoint(2, 1),
            transport: transport(),
        },
    ];
    let (plan, _) = compose(&request, instances, bindings, &components).unwrap();
    let graph = NormalizedGraph::new(plan.instances.clone(), plan.bindings.clone()).unwrap();
    assert_eq!(graph.successors(0), &[1]);
    assert_eq!(graph.bindings().len(), 3);
}
