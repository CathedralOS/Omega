use super::*;

pub(crate) fn refresh_identity(unit: &mut PsiOptimizationUnit) {
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

pub(crate) fn refresh_proof_question_identity(question: &mut ProofQuestion) {
    question.identity = omega_optimization_unit::proof_question_identity(
        question.terminal_psi,
        question.proof_bundle_fingerprint,
        question.owner,
        question.obligation,
        question.class,
        &question.proposition,
        &question.requirements,
        &question.semantic_axioms,
        question.canonical_certificate,
    );
}

pub(crate) fn refresh_node_derivatives(
    unit: &mut PsiOptimizationUnit,
    function_index: usize,
    block_index: usize,
    node_index: usize,
) {
    let block = unit.functions[function_index].blocks[block_index].id;
    let node_index = u32::try_from(node_index).expect("test node index fits u32");
    let operation = unit.functions[function_index].blocks[block_index].nodes[node_index as usize]
        .operation
        .clone();
    let node = &mut unit.functions[function_index].blocks[block_index].nodes[node_index as usize];
    node.definitions = expected_definitions(&operation, block, node_index);
    node.uses = expected_uses(&operation, block, node_index);
    node.provenance = expected_provenance(&operation);
    node.successors = expected_edges(&operation);
    node.ownership = expected_ownership(&operation);
    unit.functions[function_index].facts = reconstruct_fact_index(&unit.functions[function_index]);
    refresh_identity(unit);
}

pub(crate) fn verified_unit() -> omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    use psi_terminal::{
        Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator,
    };

    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(101, MachineId::new),
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: id(101, MachineId::new),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id(102, BlockId::new),
            blocks: vec![Block {
                id: id(102, BlockId::new),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id(103, EdgeId::new),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: id(104, psi_core::ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![
                    ContractClause {
                        obligation: id(105, psi_core::ObligationId::new),
                        proposition: Proposition::Truth,
                    },
                    ContractClause {
                        obligation: id(106, psi_core::ObligationId::new),
                        proposition: Proposition::Truth,
                    },
                ],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = psi_terminal_verifier::ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: [105, 106]
            .into_iter()
            .map(|obligation| psi_terminal_verifier::ObligationEvidence {
                obligation: id(obligation, psi_core::ObligationId::new),
                route: psi_proof_admission::EvidenceRoute::KernelDerived(
                    psi_proof_admission::PrimitiveJudgment::Truth,
                ),
            })
            .collect(),
    };
    let semantic = psi_terminal_codec::encode_module(&module).expect("encode unit module");
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).expect("encode empty proof");
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified optimizer input");
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("verified optimizer unit")
}

pub(crate) fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}
