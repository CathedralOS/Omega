use omega_psi_to_abstract_operations::{
    ProviderInstallationError, SelectedProviderAdapter, admit_provider_installation,
    lower_artifact_sections, lower_replay_artifact_sections,
    lower_replay_artifact_sections_for_optimization,
};
use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    ServiceId, StructuralTypeId,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, BoundaryMachineDeclaration, CompletionReceipt, EntryClaim, MachineContract, Operation,
    OperationKind, OperationResult, ProviderCandidateConformance, ProviderParameterRefinement,
    ProviderSignatureParameter, ProviderUnitRefinement, ProviderUnitSignature, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};
use psi_terminal_codec::{
    build_terminal_obligation_ledger, current_terminal_trust_graph, encode_module,
    encode_proof_bundle, encode_terminal_obligation_ledger, semantic_fingerprint,
};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
};
use psi_terminal_verifier::{ModuleError, ProofBundle, validate_module};

const REQUIREMENT: &str = "Signal::emit()->Unit";

#[test]
fn omega_installs_only_the_checked_adapter_selected_by_provider_plan_facts() {
    let module = provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    let obligation_ledger = build_terminal_obligation_ledger(&module, &trust_graph)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .expect("canonical obligation ledger");
    assert_eq!(
        lower_replay_artifact_sections(&semantic, &obligation_ledger, &proof, &profile)
            .expect("locally replayed artifact lowering"),
        plan
    );
    let replayed_optimizer_input = lower_replay_artifact_sections_for_optimization(
        &semantic,
        &obligation_ledger,
        &proof,
        &profile,
    )
    .expect("locally replayed optimizer input");
    assert_eq!(replayed_optimizer_input.plan(), &plan);
    assert_eq!(replayed_optimizer_input.context().module(), &module);

    let mut substituted_module = module.clone();
    let OperationKind::PortWrite { value, .. } =
        &mut substituted_module.machines[1].blocks[0].operations[0].kind
    else {
        panic!("fixture provider writes a port")
    };
    *value = 67;
    let substituted_ledger = build_terminal_obligation_ledger(&substituted_module, &trust_graph)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .expect("substituted obligation ledger");
    assert!(matches!(
        lower_replay_artifact_sections(&semantic, &substituted_ledger, &proof, &profile),
        Err(omega_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
    ));
    assert_eq!(plan.provider_candidates, module.provider_candidates);
    assert!(matches!(
        admit_provider_installation(
            &plan,
            &semantic,
            &proof,
            &profile,
            &[],
        ),
        Err(ProviderInstallationError::MissingSelectedProvider { boundary })
            if boundary == boundary_id(1)
    ));

    let selected_facts = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation =
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected_facts)
            .expect("Omega derives the exact selected terminal row");
    assert_eq!(installation.psi(), plan.psi);
    assert_eq!(
        installation.installed_candidates(),
        &plan.provider_candidates[1..]
    );
    assert_eq!(installation.installed_unit_calls().len(), 1);
    let installed_call = &installation.installed_unit_calls()[0];
    assert_eq!(installed_call.caller(), machine_id(1));
    assert_eq!(installed_call.psi_operation(), operation_id(1));
    assert_eq!(installed_call.boundary(), boundary_id(1));
    assert_eq!(installed_call.provider(), &plan.provider_candidates[1]);
    let mut execution = TerminalExecution::start_artifact_with_provider_installation(
        &semantic,
        &proof,
        &profile,
        &[],
        &[],
        installation.psi_installation(),
    )
    .expect("selected installation starts");
    assert_eq!(
        execution.resume(&mut TerminalFuelMeter::default()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(matches!(
        execution.effects(),
        [TerminalEffect::PortWrite { value: 66, .. }]
    ));

    let mut uninstalled = TerminalExecution::start_artifact(&semantic, &proof, &profile, &[])
        .expect("artifact starts without an installation");
    let mut handler = CountingEffects::default();
    assert!(matches!(
        uninstalled.resume_with_effect_handler(&mut TerminalFuelMeter::default(), &mut handler),
        Err(TerminalInterpretError::ProviderInstallationMissing(boundary))
            if boundary == boundary_id(1)
    ));
    assert_eq!(handler.calls, 0);
    assert!(uninstalled.effects().is_empty());

    let mismatched = selected("bad-plan", "FirstProvider", "SecondProvider::emit");
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &mismatched),
        Err(ProviderInstallationError::SelectedProviderMismatch { boundary })
            if boundary == boundary_id(1)
    ));
}

#[derive(Default)]
struct CountingEffects {
    calls: usize,
}

impl TerminalEffectHandler for CountingEffects {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.calls += 1;
        Ok(())
    }
}

#[test]
fn provider_catalog_identity_and_admission_fail_closed_on_tamper_or_reorder() {
    let module = provider_module();
    let original = semantic_fingerprint(&module).expect("canonical fingerprint");

    let mut identity_tamper = module.clone();
    identity_tamper.provider_candidates[1].candidate_identity = "SecondProvider::other".into();
    assert_ne!(
        semantic_fingerprint(&identity_tamper).expect("identity tamper remains representable"),
        original
    );
    let (identity_semantic, identity_proof) = artifact(&identity_tamper);
    let identity_plan = lower_artifact_sections(
        &identity_semantic,
        &identity_proof,
        &AdmissionProfile::default(),
    )
    .expect("identity-tampered artifact remains valid");
    let formerly_selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    assert!(matches!(
        admit_provider_installation(
            &identity_plan,
            &identity_semantic,
            &identity_proof,
            &AdmissionProfile::default(),
            &formerly_selected,
        ),
        Err(ProviderInstallationError::SelectedProviderMismatch { .. })
    ));

    let mut invalid = module.clone();
    invalid.provider_candidates[1]
        .signature
        .parameters
        .push(ProviderSignatureParameter {
            position: 0,
            is_self: false,
            structural_type: structural_type_id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    assert!(matches!(
        validate_module(&invalid),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
    assert!(semantic_fingerprint(&invalid).is_err());

    let mut reordered = module.clone();
    reordered.provider_candidates.swap(0, 1);
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
    assert!(encode_module(&reordered).is_err());

    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("installation for original artifact");
    let mut other = module.clone();
    let OperationKind::PortWrite { value, .. } =
        &mut other.machines[1].blocks[0].operations[0].kind
    else {
        panic!("fixture candidate writes a port")
    };
    *value = 67;
    let (other_semantic, other_proof) = artifact(&other);
    assert!(matches!(
        TerminalExecution::start_artifact_with_provider_installation(
            &other_semantic,
            &other_proof,
            &profile,
            &[],
            &[],
            installation.psi_installation(),
        ),
        Err(
            psi_terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ProviderInstallationIdentityMismatch
            )
        )
    ));
}

#[test]
fn provider_catalog_union_rejects_a_candidate_that_reenters_its_boundary() {
    let mut module = provider_module();
    module.provider_candidates.remove(0);
    module.machines.remove(1);
    module.machines[1].blocks[0].operations[0] = Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(3))
    );
}

#[test]
fn omega_retains_and_replays_the_whole_root_structural_provider_call() {
    let module = structural_provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("whole-root structural provider installation");
    let [call] = installation.installed_unit_calls() else {
        panic!("one installed structural provider call")
    };
    assert_eq!(call.provider(), &plan.provider_candidates[1]);
    assert!(call.structural_arguments()[0].path.is_empty());
    assert_eq!(
        call.structural_arguments()[0].access,
        StructuralAccess::Owned
    );
    assert_eq!(
        call.completion_receipts(),
        &[CompletionReceipt {
            claim: claim_id(1),
            argument_index: 0,
        }]
    );
    assert_eq!(call.completion_claim_sources()[0].claim, claim_id(1));

    let mut candidate_tamper = plan.clone();
    candidate_tamper.provider_candidates[1].candidate_identity = "SecondProvider::other".into();
    assert!(matches!(
        admit_provider_installation(&candidate_tamper, &semantic, &proof, &profile, &selected,),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));

    let mut access_tamper = plan.clone();
    let omega_abstract_operations::AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut access_tamper.functions[0].operations[0]
    else {
        panic!("fixture starts with its provider boundary call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        admit_provider_installation(&access_tamper, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));
}

fn selected(_name: &str, provider: &str, machine: &str) -> Vec<SelectedProviderAdapter> {
    vec![SelectedProviderAdapter {
        requirement_identity: REQUIREMENT.into(),
        provider_identity: provider.into(),
        machine_identity: machine.into(),
    }]
}

fn artifact(module: &TerminalModule) -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(module).expect("semantic section"),
        encode_proof_bundle(&ProofBundle::default()).expect("proof section"),
    )
}

fn provider_module() -> TerminalModule {
    let service = service_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: structural_type_id(1),
                identity: "named(name(FirstProvider))".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(2),
                identity: "named(name(SecondProvider))".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: vec![ServiceDeclaration {
            id: service,
            identity: "Signal".into(),
            parents: Vec::new(),
        }],
        root_service_reach: psi_terminal::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: REQUIREMENT.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: vec![
            candidate(
                "FirstProvider",
                "FirstProvider::emit",
                machine_id(2),
                service,
            ),
            candidate(
                "SecondProvider",
                "SecondProvider::emit",
                machine_id(3),
                service,
            ),
        ],
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            machine(
                machine_id(1),
                None,
                block_id(1),
                edge_id(1),
                contract_id(1),
                Operation {
                    id: operation_id(1),
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCall {
                        boundary: boundary_id(1),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        completion_receipts: Vec::new(),
                        requirement_obligations: Vec::new(),
                    },
                },
                service,
            ),
            machine(
                machine_id(2),
                Some(structural_type_id(1)),
                block_id(2),
                edge_id(2),
                contract_id(2),
                port_write(operation_id(2), service, 65),
                service,
            ),
            machine(
                machine_id(3),
                Some(structural_type_id(2)),
                block_id(3),
                edge_id(3),
                contract_id(3),
                port_write(operation_id(3), service, 66),
                service,
            ),
        ],
    }
}

fn structural_provider_module() -> TerminalModule {
    let mut module = provider_module();
    let resource = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: resource,
        identity: "Resource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let signature_parameter = ProviderSignatureParameter {
        position: 0,
        is_self: false,
        structural_type: resource,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    module.boundary_machines[0].structural_parameters =
        vec![structural_parameter(place_id(9), resource)];
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(2),
        identity: "Resource::settle".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(10), resource)],
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    for row in &mut module.provider_candidates {
        row.signature.parameters = vec![signature_parameter.clone()];
        row.refinement.positional_parameters = vec![ProviderParameterRefinement {
            boundary_index: 0,
            candidate_index: 0,
        }];
    }
    for (index, machine) in module.machines.iter_mut().enumerate() {
        let place = place_id(index as u64 + 1);
        machine.structural_parameters = vec![structural_parameter(place, resource)];
        machine.structural_places = vec![StructuralPlaceDeclaration {
            id: place,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        machine.entry_claims = vec![EntryClaim {
            claim: claim_id(1),
            input: place,
            path: Vec::new(),
        }];
        let boundary = if index == 0 {
            boundary_id(1)
        } else {
            boundary_id(2)
        };
        machine.blocks[0].operations = vec![Operation {
            id: operation_id(index as u64 + 1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                completion_receipts: vec![CompletionReceipt {
                    claim: claim_id(1),
                    argument_index: 0,
                }],
                requirement_obligations: Vec::new(),
            },
        }];
    }
    module
}

fn structural_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    }
}

fn candidate(
    provider_identity: &str,
    candidate_identity: &str,
    candidate: MachineId,
    service: ServiceId,
) -> ProviderCandidateConformance {
    ProviderCandidateConformance {
        boundary: boundary_id(1),
        requirement_identity: REQUIREMENT.into(),
        provider_identity: provider_identity.into(),
        candidate_identity: candidate_identity.into(),
        candidate,
        signature: ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: vec![service],
        },
    }
}

fn machine(
    id: MachineId,
    attachment: Option<StructuralTypeId>,
    block: BlockId,
    edge: EdgeId,
    contract: ContractId,
    operation: Operation,
    service: ServiceId,
) -> TerminalMachine {
    TerminalMachine {
        id,
        attachment,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: vec![service],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract,
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn port_write(id: OperationId, service: ServiceId, value: u8) -> Operation {
    Operation {
        id,
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: 0x80,
            value,
        },
    }
}

fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn boundary_id(value: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(value).unwrap()
}
fn structural_type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn service_id(value: u64) -> ServiceId {
    ServiceId::new(value).unwrap()
}
fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}
fn operation_id(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}
fn place_id(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
fn claim_id(value: u64) -> ClaimId {
    ClaimId::new(value).unwrap()
}
fn edge_id(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}
fn contract_id(value: u64) -> ContractId {
    ContractId::new(value).unwrap()
}
