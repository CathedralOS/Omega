//! Shared projection fixtures.

use super::*;

/// Test-only composition probe. Production target custody belongs to
/// `omega-optimization-pipeline`; these projection tests merely confirm
/// that the emitted abstract plan remains accepted by the next stage.
pub(super) struct TestLoweredOptimizedTargetOperations {
    optimized: ValidatedOptimizedAbstractPlan,
    target_operations: omega_target_operations::TargetOperationPlan,
}

impl TestLoweredOptimizedTargetOperations {
    pub(super) fn optimized(&self) -> &ValidatedOptimizedAbstractPlan {
        &self.optimized
    }

    pub(super) fn target(&self) -> NativeTarget {
        self.target_operations.target
    }

    pub(super) fn target_operations(&self) -> &omega_target_operations::TargetOperationPlan {
        &self.target_operations
    }
}

pub(super) fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<
    TestLoweredOptimizedTargetOperations,
    omega_abstract_operations_to_target_operations::LoweringError,
> {
    let target_operations =
        omega_abstract_operations_to_target_operations::lower_to_target_operations(
            optimized.plan(),
            target,
        )?;
    Ok(TestLoweredOptimizedTargetOperations {
        optimized,
        target_operations,
    })
}

pub(super) fn work_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(128, 128, 128, 128, 16).unwrap()
}

pub(super) fn verified(
    module: TerminalModule,
    mut proof: ProofBundle,
) -> VerifiedPsiOptimizationUnit {
    replace_truth_placeholders_with_checked_operation_certificates(&module, &mut proof);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

/// Replace legacy `Truth` placeholders only when the Terminal verifier has
/// reconstructed an exact canonical integer-operation obligation for that
/// evidence row. The replacement is produced by the checked-tree certificate
/// builder and is then independently admitted by the ordinary artifact path.
fn replace_truth_placeholders_with_checked_operation_certificates(
    module: &TerminalModule,
    proof: &mut ProofBundle,
) {
    let validated = psi_terminal_verifier::validate_module(module).unwrap();
    let questions = psi_terminal_verifier::reconstruct_operation_obligations(module).unwrap();
    for evidence in &mut proof.evidence {
        if evidence.route != EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth) {
            continue;
        }
        let question = questions
            .iter()
            .find(|question| question.obligation.id == evidence.obligation)
            .unwrap_or_else(|| {
                panic!(
                    "fixture Truth placeholder {} is not an operation obligation",
                    evidence.obligation
                )
            });
        assert!(
            question.canonical_certificate,
            "fixture Truth placeholder must name a canonical certificate goal"
        );
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == question.owner.machine())
            .expect("reconstructed operation owner belongs to the fixture module");
        let context = validated.value_context(machine).unwrap();
        let machine_parameter_values = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        let certificate = psi_checked_trees_to_terminal::produce_checked_canonical_integer_proof(
            &context,
            &question.obligation.proposition,
            &machine.contract.requires,
            &question.semantic_axioms,
            &machine_parameter_values,
        )
        .unwrap_or_else(|| {
            panic!(
                "fixture operation {} has no checked canonical proof",
                evidence.obligation
            )
        });
        evidence.route = EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(evidence.obligation.get()).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: certificate,
        });
    }
}

pub(super) fn checked_operation_proof_bundle(module: &TerminalModule) -> ProofBundle {
    let obligations = psi_terminal_verifier::reconstruct_operation_obligations(module).unwrap();
    let mut proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: obligations
            .iter()
            .map(|question| ObligationEvidence {
                obligation: question.obligation.id,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            })
            .collect(),
    };
    replace_truth_placeholders_with_checked_operation_certificates(module, &mut proof);
    proof
}

pub(super) fn module_with_blocks(
    machine: MachineId,
    entry: BlockId,
    result: TerminalMachineResult,
    blocks: Vec<Block>,
) -> TerminalModule {
    TerminalModule {
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks,
            contract: MachineContract {
                id: ContractId::new(machine.get() + 100).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

pub(super) fn empty_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_001).unwrap();
    let block = BlockId::new(1_002).unwrap();
    verified(
        module_with_blocks(
            machine,
            block,
            TerminalMachineResult::Unit,
            vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1_003).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
        ),
        ProofBundle::default(),
    )
}

pub(super) fn run(
    verified: VerifiedPsiOptimizationUnit,
    selections: OptimizationSelections,
) -> OptimizationRun {
    let registry = built_in_psi_registry(&selections).unwrap();
    omega_psi_optimizer::run_psi_registry(verified, &selections, &registry, work_budget()).unwrap()
}

pub(super) fn run_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: OptimizationSelections,
) -> OptimizationRun {
    run_psi_pipeline(verified, &selections, work_budget()).unwrap()
}
