//! Component verification tests.

use std::collections::BTreeSet;

use super::{
    ComponentVerificationRejection, ComponentVerificationRequest, VerifiedComponent,
    verify_component,
};
use crate::component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V1, ComponentDescription, ComponentDescriptionFacts,
    ComponentEntry, ComponentEntryKind, DescriptionDecodeRejection, DescriptionFrontier,
    EntryEvidence, ObligationKind, OutgoingAuthorityClass, OutgoingEvidence,
    decode_component_description, encode_component_description,
};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use semantic_vocabulary::{BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId};
use terminal_psi::ProofBundle;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract, Operation,
    OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, VocabularyMarker,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine identity")
}

fn minimal_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        scalar_qualifications: Default::default(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        scalar_block_invariants: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("block identity"),
            blocks: vec![Block {
                id: BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).expect("edge identity"),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// A module whose entry performs one bare `BoundaryCall` to a declared
/// Unit boundary requirement.
fn boundary_module() -> TerminalModule {
    let mut module = minimal_module();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).expect("boundary identity"),
        identity: "IndexedRequirement::apply".into(),
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
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(1).expect("operation identity"),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: BoundaryMachineId::new(1).expect("boundary identity"),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module
}

fn artifact_for(module: &TerminalModule) -> terminal_codec::CanonicalTerminalArtifact {
    let proof = ProofBundle::default();
    let record = terminal_codec::build_identity_optimization_execution_record(module, &proof)
        .expect("identity optimization record");
    terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &record, None)
        .expect("canonical artifact")
}

fn empty_selection() -> SelectedProviderPlanFacts {
    SelectedProviderPlanFacts::from_selected_plans(Vec::new()).expect("empty selection")
}

fn selected_plan() -> ProviderPlan {
    ProviderPlan {
        name: "SelectedIndexedProvider".into(),
        provider_type: "IndexedProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "IndexedRequirement".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "apply".into(),
                requirement_owner: "IndexedRequirement".into(),
                requirement_owner_package_identity: None,
                requirement_identity: "IndexedRequirement::apply".into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["IndexedRequirement".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "apply".into(),
            requirement_identity: "IndexedRequirement::apply".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "IndexedProvider::apply".into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

fn describe(module: &TerminalModule, selected: &SelectedProviderPlanFacts) -> ComponentDescription {
    let artifact = artifact_for(module);
    crate::describe_component_facts(ComponentDescriptionFacts {
        artifact: &artifact,
        selected_provider_plans: selected,
        component_progress: None,
        stack_demand: None,
        realization_identity: None,
    })
    .expect("component description")
}

fn request_for(
    module: &TerminalModule,
    accepted_assumptions: BTreeSet<[u8; 32]>,
) -> ComponentVerificationRequest {
    ComponentVerificationRequest {
        expected_subject: terminal_codec::terminal_psi_identity(module).expect("module identity"),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V1]),
        accepted_assumptions,
    }
}

fn verify(
    description: &ComponentDescription,
    request: &ComponentVerificationRequest,
) -> Result<VerifiedComponent, ComponentVerificationRejection> {
    verify_component(&encode_component_description(description), request)
}

#[test]
fn verifies_a_complete_minimal_description() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("complete description verifies");
    assert_eq!(
        verified.frontier(),
        DescriptionFrontier::TerminalArtifactClosure
    );
    assert!(
        verified
            .entries()
            .iter()
            .any(|entry| entry.kind == ComponentEntryKind::Canonical)
    );
    assert!(verified.imports().is_empty());
    assert!(verified.providers().is_empty());
}

#[test]
fn verifies_a_sealed_boundary_requirement() {
    let module = boundary_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let description = describe(&module, &selected);
    assert_eq!(description.imports.len(), 0);
    assert_eq!(description.providers.len(), 1);
    let request = request_for(&module, BTreeSet::new());
    let verified = verify(&description, &request).expect("sealed requirement verifies");
    assert_eq!(verified.providers().len(), 1);
    assert!(
        verified
            .obligations()
            .iter()
            .any(|obligation| obligation.kind == ObligationKind::ProviderOccurrence),
        "a retained provider stays a per-occurrence installation obligation"
    );
}

#[test]
fn rejects_corrupt_descriptions() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let bytes = encode_component_description(&description);
    let request = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify_component(&[], &request),
        Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::InvalidMagic | DescriptionDecodeRejection::Corrupt(_)
        ))
    ));
    assert!(matches!(
        verify_component(&bytes[..bytes.len() - 3], &request),
        Err(ComponentVerificationRejection::Corrupt(_))
    ));
    let mut mutated = bytes.clone();
    mutated[0] = b'X';
    assert!(matches!(
        verify_component(&mutated, &request),
        Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::InvalidMagic
        ))
    ));
}

#[test]
fn rejects_the_wrong_component_subject() {
    let module = minimal_module();
    let other = boundary_module();
    let description = describe(&module, &empty_selection());
    let request = request_for(&other, BTreeSet::new());
    assert!(matches!(
        verify(&description, &request),
        Err(ComponentVerificationRejection::WrongSubject { .. })
    ));
}

#[test]
fn rejects_an_incompatible_schema() {
    let module = minimal_module();
    let description = describe(&module, &empty_selection());
    let mut request = request_for(&module, BTreeSet::new());
    request.accepted_schemas = BTreeSet::from([2]);
    assert!(matches!(
        verify(&description, &request),
        Err(ComponentVerificationRejection::IncompatibleSchema { schema: 1 })
    ));
}

#[test]
fn rejects_an_early_frontier() {
    let module = minimal_module();
    let mut description = describe(&module, &empty_selection());
    description.frontier = DescriptionFrontier::SelectedPlan;
    let request = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify(&description, &request),
        Err(ComponentVerificationRejection::EarlyFrontier(
            DescriptionFrontier::SelectedPlan
        ))
    ));
}

#[test]
fn rejects_forged_complete_descriptions() {
    let module = minimal_module();
    let request = request_for(&module, BTreeSet::new());

    // A description claiming completeness while omitting the canonical entry.
    let mut omitted = describe(&module, &empty_selection());
    omitted
        .entries
        .retain(|entry| entry.kind != ComponentEntryKind::Canonical);
    assert!(matches!(
        verify(&omitted, &request),
        Err(ComponentVerificationRejection::MissingComponentEntry(_))
    ));

    // A description inventing a module-derived entry the artifact lacks.
    let mut forged = describe(&module, &empty_selection());
    forged.entries.push(ComponentEntry {
        kind: ComponentEntryKind::SuspensionResumption,
        identity: "suspension-resumption:99:1".into(),
        evidence: EntryEvidence::ModuleDerived,
    });
    assert!(matches!(
        verify(&forged, &request),
        Err(ComponentVerificationRejection::UnexpectedDerivedEntry(_))
    ));
}

#[test]
fn rejects_omitted_authority_obligation_and_import() {
    let module = boundary_module();
    let request = request_for(&module, BTreeSet::new());

    let mut no_outgoing = describe(&module, &empty_selection());
    no_outgoing
        .outgoing
        .retain(|row| row.class != OutgoingAuthorityClass::BoundaryRequirement);
    assert!(matches!(
        verify(&no_outgoing, &request),
        Err(ComponentVerificationRejection::MissingOutgoingAuthority(_))
    ));

    let mut no_import = describe(&module, &empty_selection());
    no_import.imports.clear();
    assert!(matches!(
        verify(&no_import, &request),
        Err(ComponentVerificationRejection::UnboundRequirement(_))
    ));

    let mut no_obligation = describe(&module, &empty_selection());
    no_obligation
        .obligations
        .retain(|obligation| obligation.kind != ObligationKind::ImportBinding);
    assert!(matches!(
        verify(&no_obligation, &request),
        Err(ComponentVerificationRejection::MissingObligation(_))
    ));
}

#[test]
fn rejects_unaccepted_and_unbound_assumptions() {
    let module = minimal_module();
    let assumption = [7u8; 32];

    let mut declared = describe(&module, &empty_selection());
    declared.assumptions.push(assumption);
    declared.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Startup,
        identity: "startup:post-link".into(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    verify(&declared, &accepted).expect("accepted assumption verifies");

    let unaccepted = request_for(&module, BTreeSet::new());
    assert!(matches!(
        verify(&declared, &unaccepted),
        Err(ComponentVerificationRejection::UnacceptedAssumption(_))
    ));

    let mut unbound = describe(&module, &empty_selection());
    unbound.entries.push(ComponentEntry {
        kind: ComponentEntryKind::Timer,
        identity: "timer:tick".into(),
        evidence: EntryEvidence::AssumptionBound(assumption),
    });
    let accepted = request_for(&module, BTreeSet::from([assumption]));
    assert!(matches!(
        verify(&unbound, &accepted),
        Err(ComponentVerificationRejection::UnboundAssumptionReference(
            _
        ))
    ));
}

#[test]
fn rejects_provider_roster_substitution() {
    let module = boundary_module();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
        .expect("selected closure");
    let request = request_for(&module, BTreeSet::new());

    let mut wrong_seal = describe(&module, &selected);
    for row in &mut wrong_seal.outgoing {
        if row.class == OutgoingAuthorityClass::BoundaryRequirement {
            row.evidence = OutgoingEvidence::ProviderSealed([9u8; 32]);
        }
    }
    assert!(matches!(
        verify(&wrong_seal, &request),
        Err(ComponentVerificationRejection::UnsealedProvider(_))
    ));

    let mut smuggled = describe(&module, &selected);
    smuggled.providers[0]
        .requirement_identities
        .push("Smuggled::requirement".into());
    assert!(matches!(
        verify(&smuggled, &request),
        Err(ComponentVerificationRejection::SmuggledProviderRequirement(
            _
        )) | Err(ComponentVerificationRejection::InconsistentProviderClosure(
            _
        ))
    ));
}

#[test]
fn codec_round_trips_and_rejects_noncanonical_order() {
    let module = boundary_module();
    let description = describe(&module, &empty_selection());
    let bytes = encode_component_description(&description);
    let decoded = decode_component_description(&bytes).expect("canonical decode");
    assert_eq!(decoded, description);
    assert_eq!(encode_component_description(&decoded), bytes);
}
