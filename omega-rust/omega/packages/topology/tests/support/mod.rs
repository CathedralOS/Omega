//! Shared fixtures: real admitted components built through the
//! component-description verifier, and the three-instance payments
//! composition — api calls the authorization requirement, authorization
//! provides it and calls billing's, billing provides it.
//!
//! Every roster entry named `Component` must bind an `AdmittedComponent`, so
//! nothing in this file hand-authors inventory: `verified_instance` derives
//! the component record and endpoint inventory from the verified
//! description. The `instance`/`component` helpers exist only to forge the
//! roster entries the rejection cases need.
//!
//! Each integration-test binary compiles this module separately; helpers not
//! used by a given binary are expected dead code there.
#![allow(dead_code)]

use std::collections::BTreeSet;

use component_description::test_support::{bare_module, describe_module, module_subject};
use component_description::{
    AdmissionProfile, COMPONENT_DESCRIPTION_SCHEMA_V1, ComponentVerificationRequest,
    verify_component,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    StructuralTypeId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, Operation, OperationKind, OperationResult,
    ProviderCandidateConformance, ProviderRefinement, ProviderSignature, ServiceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule, Terminator,
};
use topology_plan::*;

pub fn identity(byte: u8) -> Identity {
    [byte; 32]
}

/// The identity no byte-pattern constant can collide with: the real
/// component subjects are digests, so `identity(..)` values mark owner-side
/// policies and transports only.
pub fn verifier() -> Identity {
    identity(0x42)
}

pub fn transport() -> Identity {
    identity(0xC9)
}

pub fn name(text: &str) -> InstanceName {
    InstanceName::new(text).unwrap()
}

/// An endpoint key as a binding records it: instance ordinal plus slot.
pub fn endpoint(instance: u32, slot: u32) -> EndpointKey {
    EndpointKey { instance, slot }
}

/// A component whose entry machine calls each `calls` boundary requirement
/// and whose provider-candidate catalog realizes each `(requirement,
/// provider, candidate)` triple. This is the smallest real inventory a
/// verified description can carry.
pub fn component_module(calls: &[&str], provides: &[(&str, &str, &str)]) -> TerminalModule {
    let mut module = bare_module();
    let mut boundary_id = 1u64;
    for (operation_id, requirement) in (1u64..).zip(calls.iter()) {
        let boundary = BoundaryMachineId::new(boundary_id).expect("boundary identity");
        boundary_id += 1;
        module
            .boundary_machines
            .push(boundary_machine(boundary, requirement));
        module.machines[0].blocks[0].operations.push(Operation {
            static_reach_binding: None,
            id: OperationId::new(operation_id).expect("operation identity"),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        });
    }
    for (index, &(requirement, provider, candidate)) in provides.iter().enumerate() {
        // A requirement the module already calls is the same boundary:
        // realize that declaration rather than duplicating it.
        let boundary = match module
            .boundary_machines
            .iter()
            .find(|declared| declared.identity == requirement)
        {
            Some(declared) => declared.id,
            None => {
                let fresh = BoundaryMachineId::new(boundary_id).expect("boundary identity");
                boundary_id += 1;
                module
                    .boundary_machines
                    .push(boundary_machine(fresh, requirement));
                fresh
            }
        };

        let provider_type = StructuralTypeId::new(index as u64 + 1).expect("structural type");
        module.structural_types.push(StructuralTypeDeclaration {
            id: provider_type,
            identity: provider.to_owned(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        });
        let candidate_id = index as u64 + 2;
        let mut machine = module.machines[0].clone();
        // The clone carries the entry machine's boundary calls; operation
        // identities are module-unique, so the candidate runs a bare block.
        machine.blocks[0].operations = Vec::new();
        machine.id = MachineId::new(candidate_id).expect("machine identity");
        machine.attachment = Some(provider_type);
        machine.contract.id = ContractId::new(candidate_id).expect("contract");
        machine.entry = BlockId::new(candidate_id).expect("block");
        machine.blocks[0].id = machine.entry;
        machine.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(candidate_id).expect("edge"),
            trivial_affine_discards: Vec::new(),
        };
        let candidate_machine = machine.id;
        module.machines.push(machine);
        module
            .provider_candidates
            .push(ProviderCandidateConformance {
                boundary,
                requirement_identity: requirement.to_owned(),
                provider_identity: provider.to_owned(),
                candidate_identity: candidate.to_owned(),
                candidate: candidate_machine,
                signature: ProviderSignature {
                    parameters: Vec::new(),
                },
                refinement: ProviderRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: Vec::new(),
                },
            });
    }
    module
}

fn boundary_machine(id: BoundaryMachineId, requirement: &str) -> BoundaryMachineDeclaration {
    BoundaryMachineDeclaration {
        id,
        identity: requirement.to_owned(),
        attachment: None,
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    }
}

/// Verify `module`'s description under the consumer fence the tests use —
/// the v1 schema and an empty admission profile — and keep the exact request
/// so the admission's profile identity is recoverable.
pub fn admit(module: &TerminalModule) -> AdmittedComponent {
    admit_with(module, BTreeSet::new())
}

/// `admit` under a request that accepts these assumption digests.
pub fn admit_with(
    module: &TerminalModule,
    accepted_assumptions: BTreeSet<[u8; 32]>,
) -> AdmittedComponent {
    let request = ComponentVerificationRequest {
        expected_subject: module_subject(module),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V1]),
        accepted_assumptions,
        admission_profile: AdmissionProfile::default(),
    };
    let component = verify_component(&describe_module(module), &request)
        .expect("the component verifies under the request");
    AdmittedComponent { request, component }
}

/// The component subject every admission binds, in order — the roster
/// owner intent names.
pub fn subjects_of(components: &[AdmittedComponent]) -> Vec<Identity> {
    components.iter().map(subject_of).collect()
}

/// The plan-level identity of one admission's subject — what a request
/// roster member must name to bind it.
pub fn subject_of(admission: &AdmittedComponent) -> Identity {
    component_subject_identity(&admission.component.subject())
}

/// The port a test component writes: one physical mechanism assumption the
/// description demands.
pub const PORT_SERVICE: u64 = 7;
pub const PORT_NUMBER: u16 = 0x3F8;
pub const PORT_VALUE: u8 = 0x41;

/// api plus one raw port write — a real assumption-bearing component. Its
/// description's assumption roster names the port-mechanism digest, so this
/// is the honest way a demanded assumption reaches a plan.
pub fn assumption_api_module() -> (TerminalModule, Identity) {
    let service = ServiceId::new(PORT_SERVICE).expect("service identity");
    let mut module = api_module();
    module.services.push(ServiceDeclaration {
        id: service,
        identity: "PortService".to_owned(),
        parents: Vec::new(),
    });
    // The write is only valid within the entry machine's published ceiling,
    // and the module's root reach must declare what it derives.
    module.machines[0].published_service_ceiling = vec![service];
    module.root_service_reach.concrete = vec![service];
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(9).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: PORT_NUMBER,
            value: PORT_VALUE,
        },
    });
    let assumption =
        component_description::port_mechanism_assumption(service, PORT_NUMBER, PORT_VALUE);
    (module, assumption)
}

// ---- the payments roster --------------------------------------------------

/// api: demands the authorization requirement and exports only its
/// canonical entry.
pub fn api_module() -> TerminalModule {
    component_module(&["AuthorizationBoundary::authorize"], &[])
}

/// authorization: realizes the authorization requirement for callers and
/// demands billing's.
pub fn authorization_module() -> TerminalModule {
    component_module(
        &["BillingBoundary::post"],
        &[(
            "AuthorizationBoundary::authorize",
            "AuthorizationProvider",
            "AuthorizationProvider::authorize",
        )],
    )
}

/// billing: realizes the billing requirement.
pub fn billing_module() -> TerminalModule {
    component_module(
        &[],
        &[(
            "BillingBoundary::post",
            "BillingProvider",
            "BillingProvider::post",
        )],
    )
}

/// The three payment components' admissions, in roster order.
pub fn payment_components() -> Vec<AdmittedComponent> {
    vec![
        admit(&api_module()),
        admit(&authorization_module()),
        admit(&billing_module()),
    ]
}

/// A forged component record — arbitrary digests with no establishing
/// artifact. Used only where the case needs a record that rejects.
pub fn component(subject: Identity) -> ComponentDescription {
    ComponentDescription {
        subject,
        verification_profile: identity(0x20),
        completeness: Completeness::VerifiedComplete {
            closure: identity(0x30),
        },
        assumptions: vec![],
    }
}

/// A forged roster entry — arbitrary record plus hand-listed import/export
/// slots. Real inventory comes from `verified_instance`; this exists for
/// mutations that must reach the roster or graph checks.
pub fn instance(name: &str, subject: u8, imports: &[u32], exports: &[u32]) -> PlanInstance {
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
    endpoints.sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
    PlanInstance {
        name: self::name(name),
        component: component(identity(subject)),
        role: InstanceRole::Component,
        endpoints,
    }
}

/// The real payment roster: every record is reconstructed from an admission.
pub fn payment_instances() -> Vec<PlanInstance> {
    let components = payment_components();
    vec![
        verified_instance(name("api"), &components[0]),
        verified_instance(name("authorization"), &components[1]),
        verified_instance(name("billing"), &components[2]),
    ]
}

pub fn payment_bindings() -> Vec<Binding> {
    vec![
        Binding {
            // api's demanded import (slot 0) -> authorization's requirement
            // export (slot 1, after the canonical entry export on slot 0).
            import: EndpointKey {
                instance: 0,
                slot: 0,
            },
            export: EndpointKey {
                instance: 1,
                slot: 1,
            },
            transport: transport(),
        },
        Binding {
            // authorization's demanded import -> billing's requirement export.
            import: EndpointKey {
                instance: 1,
                slot: 0,
            },
            export: EndpointKey {
                instance: 2,
                slot: 1,
            },
            transport: transport(),
        },
    ]
}

pub fn payment_policies() -> Vec<PolicyCall> {
    vec![
        PolicyCall::no_route(
            PolicySelector::new([name("billing")]),
            PolicySelector::new([name("api")]),
        ),
        PolicyCall::only_via(
            PolicySelector::new([name("api")]),
            PolicySelector::new([name("billing")]),
            PolicySelector::new([name("authorization")]),
        ),
    ]
}

pub fn payment_request() -> TopologyRequest {
    let components = payment_components();
    TopologyRequest {
        instances: vec![
            RequestedInstance {
                name: name("api"),
                subject: subject_of(&components[0]),
            },
            RequestedInstance {
                name: name("authorization"),
                subject: subject_of(&components[1]),
            },
            RequestedInstance {
                name: name("billing"),
                subject: subject_of(&components[2]),
            },
        ],
        policies: payment_policies(),
        verifier: verifier(),
        transports: vec![transport()],
        accepted_assumptions: vec![],
    }
}

/// Compose the golden plan and its request bytes from the real admissions.
pub fn payment_pair() -> (Vec<u8>, Vec<u8>, DeploymentPlan) {
    let request = payment_request();
    let request_bytes = encode_request(&request).unwrap();
    let components = payment_components();
    let (plan, _outcomes) = compose_plan(
        &request,
        &request_bytes,
        payment_instances(),
        payment_bindings(),
        verifier(),
        &components,
    )
    .expect("payment composition succeeds");
    let plan_bytes = encode_plan(&plan).unwrap();
    (request_bytes, plan_bytes, plan)
}
