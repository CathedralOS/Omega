//! Real verifier custody for an unranked loop with no arithmetic or proof obligations.
use super::validate_unit_custody;
use semantic_vocabulary::{BlockId, ContractId, EdgeId, FuelScheduleIdentity, MachineId};
use terminal_psi::{
    Block, MachineContract, ProofBundle, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationUnit, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};

fn verified(backedge_ordinal: u64) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let header = BlockId::new(2).unwrap();
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
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
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
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
            entry,
            blocks: [(entry, 1), (header, backedge_ordinal)]
                .map(|(id, edge)| Block {
                    id,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(edge).unwrap(),
                        target: header,
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                })
                .to_vec(),
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let input = lower_artifact_sections_for_optimization(
        &terminal_codec::encode_module(&module).unwrap(),
        &terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    build_verified_psi_optimization_unit(input, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

fn target(source: &VerifiedPsiOptimizationUnit) -> target_operations::TargetOperationPlan {
    abstract_operations_to_target_operations::lower_to_target_operations(
        source.input().plan(),
        target::NativeTarget::macos_arm64(),
    )
    .unwrap()
}

#[test]
fn verified_unranked_cycle_has_legalizer_custody_without_rank_evidence() {
    let source = verified(2);
    assert!(
        source.input().context().module().machines[0]
            .ranked_scc
            .is_none()
    );
    assert!(
        source
            .input()
            .context()
            .proof_bundle()
            .control_cycles
            .is_empty()
    );
    validate_unit_custody(
        &target(&source),
        source.input().plan(),
        source.unit(),
        Some(source.input()),
    )
    .unwrap();
}

#[test]
fn unranked_cycle_still_rejects_without_verified_source() {
    let source = verified(2);
    assert!(
        validate_unit_custody(&target(&source), source.input().plan(), source.unit(), None)
            .is_err()
    );
}

#[test]
fn unranked_cycle_rejects_a_different_verified_source() {
    let source = verified(2);
    let other = verified(3);
    assert_ne!(source.input().plan().psi, other.input().plan().psi);
    assert!(
        validate_unit_custody(
            &target(&source),
            source.input().plan(),
            source.unit(),
            Some(other.input())
        )
        .is_err()
    );
}

#[test]
fn unranked_cycle_rejects_coherent_current_backedge_redirection() {
    let source = verified(2);
    let mut changed = source.unit().clone();
    let entry = changed.functions[0].entry;
    let node = &mut changed.functions[0].blocks[1].nodes[0];
    let abstract_operations::AbstractOperation::Jump { target, .. } = &mut node.operation else {
        panic!("loop jump");
    };
    *target = entry;
    node.successors[0].target = entry;
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(
        validate_unit_custody(
            &self::target(&source),
            source.input().plan(),
            &changed,
            Some(source.input())
        )
        .is_err()
    );
}
