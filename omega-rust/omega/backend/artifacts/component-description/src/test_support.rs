//! Canonical component-module fixtures for consumers of verified
//! descriptions.
//!
//! A consumer test needs a real described-and-verifiable component whose
//! provider-candidate catalog names exact selected-plan coordinates. These
//! builders produce such modules directly in Terminal Psi so a build route
//! can describe them with `describe_component_facts` and admit them with
//! `verify_component`; nothing here is a hand-authored inventory.

use std::collections::BTreeSet;

use effects::SelectedProviderPlanFacts;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, StructuralTypeId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract,
    ProviderCandidateConformance, ProviderRefinement, ProviderSignature, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalPsiIdentity, Terminator, VocabularyMarker,
};

use crate::{
    AdmissionProfile, COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescriptionFacts,
    ComponentVerificationRequest, VerifiedComponent, describe_component_facts,
    encode_component_description, verify_component,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine identity")
}

/// A canonical Terminal module with one Unit entry machine and nothing else:
/// it describes a component that exports no realization.
pub fn bare_module() -> TerminalModule {
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
        operation_crash_contracts: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// A provider component module: it declares the boundary requirement and
/// retains one checked candidate machine, attached to the provider type,
/// realizing that requirement with the given exact coordinates. Those are
/// the coordinates `VerifiedComponent::realizes_selected_plan` joins against
/// a selected plan's requirement identity, provider type, and checked
/// adapter machine identity.
pub fn provider_module(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> TerminalModule {
    let mut module = bare_module();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).expect("boundary identity"),
        identity: requirement_identity.to_owned(),
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
    let provider_type = StructuralTypeId::new(1).expect("structural type identity");
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: provider_identity.to_owned(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut candidate = module.machines[0].clone();
    candidate.id = machine_id(2);
    candidate.attachment = Some(provider_type);
    candidate.contract.id = ContractId::new(2).expect("contract identity");
    candidate.entry = BlockId::new(2).expect("block identity");
    candidate.blocks[0].id = candidate.entry;
    candidate.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(2).expect("edge identity"),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(candidate);
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(1).expect("boundary identity"),
            requirement_identity: requirement_identity.to_owned(),
            provider_identity: provider_identity.to_owned(),
            candidate_identity: candidate_identity.to_owned(),
            candidate: machine_id(2),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module
}

/// The Terminal Psi identity a compilation of `module` observes: the exact
/// subject a consumer must supply to admit the module's description.
pub fn module_subject(module: &TerminalModule) -> TerminalPsiIdentity {
    terminal_codec::terminal_psi_identity(module).expect("module identity")
}

/// Describe one component module the way a producing compilation would:
/// canonical artifact, identity optimization record, no sealed requirement
/// of its own, and no native realization facts. Returns the encoded
/// description bytes a consumer verifies.
pub fn describe_module(module: &TerminalModule) -> Vec<u8> {
    let proof = terminal_psi::ProofBundle::default();
    let record = terminal_codec::build_identity_optimization_execution_record(module, &proof)
        .expect("identity optimization record");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &record, None)
            .expect("canonical artifact");
    let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new())
        .expect("a provider component seals no requirement of its own");
    let description = describe_component_facts(ComponentDescriptionFacts {
        artifact: &artifact,
        selected_provider_plans: &selected,
        component_progress: None,
        stack_demand: None,
        realization_identity: None,
    })
    .expect("component description");
    encode_component_description(&description)
}

/// Describe and independently verify one component module under the
/// current schema with no accepted assumptions and an empty module-proof
/// admission profile: the producer/consumer pair a build route runs, so a
/// consumer test only ever sees replayed evidence.
pub fn verified_component(module: &TerminalModule) -> VerifiedComponent {
    let request = ComponentVerificationRequest {
        expected_subject: module_subject(module),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V2]),
        accepted_assumptions: BTreeSet::new(),
        admission_profile: AdmissionProfile::default(),
    };
    verify_component(&describe_module(module), &request).expect("the component verifies")
}
