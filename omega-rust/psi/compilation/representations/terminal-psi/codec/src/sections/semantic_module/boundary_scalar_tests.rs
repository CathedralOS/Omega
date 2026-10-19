//! Scalar boundary declaration identity and exact actual-argument wire joins.
use super::module_wire::encode_raw;
use super::routed_scalar_tests::module;
use super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    DomainSemanticId, IntegerSign, IntegerType, ScalarDomainId, ScalarQualificationSetId,
    ScalarType, ValueId,
};
use terminal_psi::{ScalarReturnOrigin, TerminalIdentityIndex, TerminalModule};

// Scalar boundary contracts share the same catalog as ordinary signatures.
// No source predicate proof is reconstructed by these wire-format controls.
pub(super) fn boundary_parameter_module() -> TerminalModule {
    let mut module = module();
    module.identities = identity_table(7);
    let carrier = module.machines[0].parameters[0].scalar_type;
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
    module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            declaration: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(5),
            owner: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(6)),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
            scalar_parameters: vec![carrier],
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            scalar_domain_requirements: vec![terminal_psi::ScalarDomainRequirement {
                argument_index: 0,
                domain: ScalarDomainId::new(1).unwrap(),
            }],
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_service_ceiling: Vec::new(),
        });
    module.machines[0].blocks[0]
        .operations
        .push(terminal_psi::Operation {
            id: semantic_vocabulary::OperationId::new(1).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: terminal_psi::OperationResult::Unit,
            kind: terminal_psi::OperationKind::BoundaryCall {
                boundary,
                arguments: vec![ValueId::new(1).unwrap()],
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        });
    module
}

#[test]
fn scalar_boundary_membership_roundtrips_with_exact_call_subject() {
    let original = boundary_parameter_module();
    let bytes = encode_module(&original).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(original, decoded);
    let mut changed = original.clone();
    changed.boundary_machines[0]
        .scalar_domain_requirements
        .clear();
    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn scalar_boundary_membership_rejects_detached_nominal_transport() {
    let original = boundary_parameter_module();
    let rejects =
        |module: &TerminalModule| assert!(decode_module(&encode_raw(module).unwrap()).is_err());
    let mut changed = original.clone();
    changed.boundary_machines[0].scalar_domain_requirements[0].argument_index = 1;
    rejects(&changed);
    let mut changed = original.clone();
    changed.boundary_machines[0].scalar_domain_requirements[0].domain =
        ScalarDomainId::new(99).unwrap();
    rejects(&changed);
    let mut changed = original.clone();
    let duplicate = changed.boundary_machines[0].scalar_domain_requirements[0];
    changed.boundary_machines[0]
        .scalar_domain_requirements
        .push(duplicate);
    rejects(&changed);
    let mut changed = original.clone();
    changed.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    changed.scalar_qualifications.returns[0].origins =
        vec![ScalarReturnOrigin::Established { route: 0 }];
    let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("boundary scalar argument lacks"),
        "{failure}"
    );
    let mut changed = original.clone();
    let mut other = changed.scalar_qualifications.domains[0].clone();
    other.id = ScalarDomainId::new(2).unwrap();
    other.semantic_domain = DomainSemanticId::new(2).unwrap();
    other.identity = TerminalIdentityIndex(7);
    changed.scalar_qualifications.domains.push(other);
    changed.boundary_machines[0].scalar_domain_requirements[0].domain =
        ScalarDomainId::new(2).unwrap();
    rejects(&changed);
    let mut changed = original;
    changed.boundary_machines[0].scalar_parameters[0] =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    rejects(&changed);
}

#[test]
fn scalar_provider_cannot_strengthen_boundary_membership() {
    let mut original = boundary_parameter_module();
    original.identities = identity_table(10);
    let provider_id = semantic_vocabulary::MachineId::new(2).unwrap();
    let mut provider = unit_machine(provider_id);
    provider.declaration.as_mut().unwrap().callable = TerminalIdentityIndex(8);
    provider.parameters = original.machines[0].parameters.clone();
    original.machines.push(provider);
    original
        .provider_candidates
        .push(terminal_psi::ProviderCandidateConformance {
            boundary: original.boundary_machines[0].id,
            requirement_identity: original.boundary_machines[0].identity,
            source_requirement_identity: original.boundary_machines[0].identity,
            receiving_application: TerminalIdentityIndex::ABSENT,
            receiving_region_arguments: Vec::new(),
            provider_identity: TerminalIdentityIndex(9),
            provider_declaration: terminal_psi::NominalDeclarationIdentity::new(
                TerminalIdentityIndex(9),
            ),
            candidate_identity: TerminalIdentityIndex(8),
            source_candidate_identity: TerminalIdentityIndex(8),
            source_candidate_package_identity: None,
            candidate: provider_id,
            signature: terminal_psi::ProviderSignature::empty(),
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: vec![terminal_psi::ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                }],
                ..terminal_psi::ProviderRefinement::empty()
            },
        });
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
    let mut changed = original;
    let mut other = changed.scalar_qualifications.domains[0].clone();
    other.id = ScalarDomainId::new(2).unwrap();
    other.semantic_domain = DomainSemanticId::new(2).unwrap();
    other.identity = TerminalIdentityIndex(10);
    changed.scalar_qualifications.domains.push(other);
    changed
        .scalar_qualifications
        .sets
        .push(terminal_psi::ScalarQualificationSet {
            id: ScalarQualificationSetId::new(2),
            domains: vec![
                ScalarDomainId::new(1).unwrap(),
                ScalarDomainId::new(2).unwrap(),
            ],
        });
    changed.machines[1].parameters[0].qualifications = ScalarQualificationSetId::new(2);
    let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("provider candidate requires scalar membership absent"),
        "{failure}"
    );
}
