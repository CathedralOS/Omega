//! Producer-side composition coverage: the payment graph succeeds; direct and
//! indirect bypasses, cycles, disconnection, duplicates, incomplete bindings,
//! and invalid policy inputs all reject or violate with checked witnesses —
//! never silently.

mod support;

use support::*;
use topology_plan::*;

fn compose(
    request: &TopologyRequest,
    instances: Vec<PlanInstance>,
    bindings: Vec<Binding>,
) -> Result<(DeploymentPlan, Vec<PolicyOutcome>), CompositionError> {
    let request_bytes = encode_request(request).unwrap();
    compose_plan(request, &request_bytes, instances, bindings, verifier())
}

fn add_import(instance: &mut PlanInstance, slot: u32) {
    instance.endpoints.push(Endpoint {
        slot,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
}

fn add_export(instance: &mut PlanInstance, slot: u32) {
    instance.endpoints.push(Endpoint {
        slot,
        direction: EndpointDirection::Export,
        contract: identity(0xC1),
    });
}

#[test]
fn the_payment_graph_composes() {
    let request = payment_request();
    let (plan, outcomes) = compose(&request, payment_instances(), payment_bindings())
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
    let request = payment_request();
    let mut instances = payment_instances();
    add_import(&mut instances[0], 2);
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(0, 2),
        export: endpoint(2, 1),
        transport: transport(),
    });
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings)
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
    let request = payment_request();
    let mut instances = payment_instances();
    instances.push(instance("logging", 0x44, &[1], &[1]));
    // Extend the request roster for logging.
    let mut request = request;
    request.instances.push(RequestedInstance {
        name: name("logging"),
        subject: identity(0x44),
    });
    add_import(&mut instances[0], 2);
    add_import(&mut instances[3], 1); // already has 1; add a second demanded slot
    instances[3].endpoints.pop(); // keep exactly one demanded import
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(0, 2),
        export: endpoint(3, 1),
        transport: transport(),
    });
    bindings.push(Binding {
        import: endpoint(3, 1),
        export: endpoint(2, 1),
        transport: transport(),
    });
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings)
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
    let request = payment_request();
    let mut instances = payment_instances();
    // authorization -> api closes a cycle without touching the policies:
    // billing still reaches nothing, and api -> billing still must pass
    // through authorization.
    add_import(&mut instances[1], 9);
    add_export(&mut instances[0], 9);
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(1, 9),
        export: endpoint(0, 9),
        transport: transport(),
    });
    compose(&request, instances, bindings).expect("cyclic graph composes");
}

#[test]
fn an_unbound_demanded_import_rejects_before_policies() {
    let request = payment_request();
    let mut instances = payment_instances();
    add_import(&mut instances[0], 2); // demanded, never bound
    let Err(CompositionError::Rejected(CompositionFailure::InvalidGraph(
        GraphError::UnboundImport { .. },
    ))) = compose(&request, instances, payment_bindings())
    else {
        panic!("unbound import must reject")
    };
}

#[test]
fn duplicate_bindings_reject() {
    let request = payment_request();
    let mut bindings = payment_bindings();
    bindings.push(bindings[0].clone());
    let Err(CompositionError::Rejected(CompositionFailure::InvalidGraph(
        GraphError::DuplicateBinding { .. },
    ))) = compose(&request, payment_instances(), bindings)
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
    let mut instances = payment_instances();
    instances.push(instances[0].clone());
    assert!(matches!(
        compose(&request, instances, payment_bindings()),
        Err(CompositionError::Rejected(
            CompositionFailure::RosterMismatch { .. }
        ))
    ));
}

#[test]
fn compatible_endpoints_with_no_connection_gain_no_edge() {
    // billing and authorization both carry compatible contracts but nothing
    // connects billing back to authorization.
    let request = payment_request();
    let (plan, _) = compose(&request, payment_instances(), payment_bindings()).unwrap();
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
    let mut request = payment_request();
    // Sorted roster: api < api-east < authorization < billing.
    request.instances.insert(
        1,
        RequestedInstance {
            name: name("api-east"),
            subject: identity(0x11), // same component subject as api
        },
    );
    request.policies = vec![PolicyCall::no_route(
        PolicySelector::new([name("api-east")]),
        PolicySelector::new([name("billing")]),
    )];
    // Sorted instances: api(0) api-east(1) authorization(2) billing(3).
    let mut instances = payment_instances();
    let api_east = instance("api-east", 0x11, &[1], &[]);
    instances.insert(1, api_east);
    let bindings = vec![
        Binding {
            import: endpoint(0, 1),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 1),
            export: endpoint(2, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(2, 1),
            export: endpoint(3, 1),
            transport: transport(),
        },
    ];
    // api-east -> authorization -> billing: no_route({api-east},{billing})
    // must still fail — instance identity, not code, carries the policy.
    let Err(CompositionError::PoliciesViolated { evaluations, .. }) =
        compose(&request, instances, bindings)
    else {
        panic!("api-east route must violate no_route")
    };
    assert!(evaluations[0].0.contains("no_route"));
}

#[test]
fn a_self_connection_is_allowed_and_visible() {
    let mut request = payment_request();
    request.instances.push(RequestedInstance {
        name: name("scheduler"),
        subject: identity(0x55),
    });
    let mut instances = payment_instances();
    let mut scheduler = instance("scheduler", 0x55, &[1], &[1]);
    scheduler
        .endpoints
        .sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
    instances.push(scheduler);
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(3, 1),
        export: endpoint(3, 1),
        transport: transport(),
    });
    compose(&request, instances, bindings).expect("self-connection composes");
}

#[test]
fn swapped_component_code_fails_the_roster() {
    let request = payment_request();
    let mut instances = payment_instances();
    instances[2].component.subject = identity(0x34);
    assert!(matches!(
        compose(&request, instances, payment_bindings()),
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
        );
        assert!(
            matches!(result, Err(CompositionError::PoliciesViolated { .. })),
            "invalid selector must reject"
        );
    }
}

#[test]
fn multiple_edges_between_the_same_pair_keep_all_evidence() {
    // Two distinct demanded imports on api may both bind to authorization:
    // the adjacency deduplicates for traversal while both binding rows stay.
    let request = payment_request();
    let mut instances = payment_instances();
    add_import(&mut instances[0], 2);
    add_export(&mut instances[1], 2);
    let mut bindings = payment_bindings();
    bindings.push(Binding {
        import: endpoint(0, 2),
        export: endpoint(1, 2),
        transport: transport(),
    });
    let (plan, _) = compose(&request, instances, bindings).unwrap();
    let graph = NormalizedGraph::new(plan.instances.clone(), plan.bindings.clone()).unwrap();
    assert_eq!(graph.successors(0), &[1]);
    assert_eq!(graph.bindings().len(), 3);
}
