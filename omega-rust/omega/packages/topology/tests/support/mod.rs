//! Shared fixtures: the payment composition from the topology contract —
//! api -> authorization -> billing under `only_via({api},{billing},{authorization})`.
//!
//! Each integration-test binary compiles this module separately; helpers not
//! used by a given binary are expected dead code there.
#![allow(dead_code)]

use topology_plan::*;

pub const VERIFIER_BYTE: u8 = 0x50;
pub const TRANSPORT_BYTE: u8 = 0x77;

pub fn identity(byte: u8) -> Identity {
    [byte; 32]
}

pub fn verifier() -> Identity {
    identity(VERIFIER_BYTE)
}

pub fn transport() -> Identity {
    identity(TRANSPORT_BYTE)
}

pub fn name(text: &str) -> InstanceName {
    InstanceName::new(text).unwrap()
}

/// A component description whose closure is verifier-established. `subject`
/// distinguishes components; two instances may share it deliberately.
pub fn component(subject: u8) -> ComponentDescription {
    ComponentDescription {
        subject: identity(subject),
        verification_profile: identity(0x20),
        completeness: Completeness::VerifiedComplete {
            closure: identity(0x30),
        },
        assumptions: Vec::new(),
    }
}

/// An instance with `imports` demanded and `exports` offered, all on distinct
/// slots starting at `base`.
pub fn instance(name_text: &str, subject: u8, imports: &[u32], exports: &[u32]) -> PlanInstance {
    let mut endpoints = Vec::new();
    for &slot in imports {
        endpoints.push(Endpoint {
            slot,
            direction: EndpointDirection::Import,
            contract: identity(0xC0),
        });
    }
    for &slot in exports {
        endpoints.push(Endpoint {
            slot,
            direction: EndpointDirection::Export,
            contract: identity(0xC1),
        });
    }
    PlanInstance {
        name: name(name_text),
        component: component(subject),
        role: InstanceRole::Component,
        endpoints,
    }
}

pub fn endpoint(instance_index: u32, slot: u32) -> EndpointKey {
    EndpointKey {
        instance: instance_index,
        slot,
    }
}

/// api(0) -> authorization(1) -> billing(2) on slot 1.
pub fn payment_instances() -> Vec<PlanInstance> {
    vec![
        instance("api", 0x11, &[1], &[]),
        instance("authorization", 0x22, &[1], &[1]),
        instance("billing", 0x33, &[], &[1]),
    ]
}

pub fn payment_bindings() -> Vec<Binding> {
    vec![
        Binding {
            import: endpoint(0, 1),
            export: endpoint(1, 1),
            transport: transport(),
        },
        Binding {
            import: endpoint(1, 1),
            export: endpoint(2, 1),
            transport: transport(),
        },
    ]
}

/// The required policy set: `only_via({api},{billing},{authorization})` plus a
/// `no_route({billing},{api})` demonstrating deliberate disconnection.
pub fn payment_policies() -> Vec<PolicyCall> {
    let mut calls = vec![
        PolicyCall::no_route(
            PolicySelector::new([name("billing")]),
            PolicySelector::new([name("api")]),
        ),
        PolicyCall::only_via(
            PolicySelector::new([name("api")]),
            PolicySelector::new([name("billing")]),
            PolicySelector::new([name("authorization")]),
        ),
    ];
    calls.sort_by_key(|call| call.canonical_key());
    calls
}

pub fn payment_request() -> TopologyRequest {
    TopologyRequest {
        instances: vec![
            RequestedInstance {
                name: name("api"),
                subject: identity(0x11),
            },
            RequestedInstance {
                name: name("authorization"),
                subject: identity(0x22),
            },
            RequestedInstance {
                name: name("billing"),
                subject: identity(0x33),
            },
        ],
        policies: payment_policies(),
        verifier: verifier(),
        transports: vec![transport()],
        accepted_assumptions: Vec::new(),
    }
}

/// The checked golden pair: request bytes and the plan composed for them.
pub fn payment_pair() -> (Vec<u8>, Vec<u8>, DeploymentPlan) {
    let request = payment_request();
    let request_bytes = encode_request(&request).unwrap();
    let (plan, _outcomes) = compose_plan(
        &request,
        &request_bytes,
        payment_instances(),
        payment_bindings(),
        verifier(),
    )
    .expect("payment composition succeeds");
    let plan_bytes = encode_plan(&plan).unwrap();
    (request_bytes, plan_bytes, plan)
}
