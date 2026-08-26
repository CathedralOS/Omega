#![forbid(unsafe_code)]

//! Fail-closed orchestration for explicit, nonempty optimization selections.
//!
//! The ordinary empty-selection compiler path does not call this crate. This
//! entry point begins with the verified Terminal-Psi artifact boundary, runs
//! every selected named pass under the same explicit per-pass work ceiling,
//! and returns only the custody-preserving optimized abstract-plan carrier.

use omega_lowering_optimizer::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, project_optimization_run,
};
use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_psi_optimizer::{OptimizationRunError, run_psi_pipeline};
use omega_terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationUnitBuildError,
    VerifiedTerminalOptimizationInput, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;

mod assignment;

pub use assignment::{
    OptimizedAssignmentCustodyError, OptimizedAssignmentPipelineError,
    StagedOptimizedAssignedOperations, StagedOptimizedAssignmentCustodyReceipt,
    stage_optimized_assignment, stage_optimized_assignment_with_provider_executions,
    validate_optimized_assignment_custody,
};

/// Exact optimizer inputs chosen by compiler orchestration.
///
/// Construction rejects the empty selection so compatibility builds cannot
/// accidentally enter this crate or manufacture optimizer work records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitOptimizationRequest {
    selections: OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
}

impl ExplicitOptimizationRequest {
    pub fn new(
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Result<Self, EmptyOptimizationSelections> {
        if selections.is_empty() {
            return Err(EmptyOptimizationSelections);
        }
        Ok(Self {
            selections,
            budget_per_pass,
        })
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }
}

/// Compiler-owned bounded baseline for the experimental optimized lane.
/// Every value is a per-pass-group ceiling; this is not a source-visible
/// optimization level or an intensity preset.
pub fn compiler_baseline_request_v1(
    selections: &OptimizationSelections,
) -> Result<ExplicitOptimizationRequest, EmptyOptimizationSelections> {
    ExplicitOptimizationRequest::new(
        selections.clone(),
        OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 10_000)
            .expect("compiler baseline optimizer ceilings are nonzero"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOptimizationSelections;

impl std::fmt::Display for EmptyOptimizationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the explicit optimizer pipeline requires at least one named selection")
    }
}

impl std::error::Error for EmptyOptimizationSelections {}

#[derive(Debug)]
pub enum OptimizationPipelineError {
    ArtifactLowering(ArtifactLoweringError),
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    AbstractProjection(OptimizedAbstractProjectionError),
}

impl std::fmt::Display for OptimizationPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "explicit optimization pipeline failed: {self:?}")
    }
}

impl std::error::Error for OptimizationPipelineError {}

pub fn optimize_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &AdmissionProfile,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let input = lower_artifact_sections_for_optimization(semantic_bytes, proof_bytes, profile)
        .map_err(OptimizationPipelineError::ArtifactLowering)?;
    optimize_verified_terminal_input(input, request)
}

pub fn optimize_verified_terminal_input(
    input: VerifiedTerminalOptimizationInput,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(OptimizationPipelineError::UnitBuild)?;
    let run = run_psi_pipeline(verified, request.selections(), request.budget_per_pass())
        .map_err(OptimizationPipelineError::Run)?;
    project_optimization_run(run).map_err(OptimizationPipelineError::AbstractProjection)
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{Optimization, OptimizationSelections};
    use omega_psi_optimizer::{OptimizationRunError, RuleRegistryError};
    use omega_target::NativeTarget;
    use omega_terminal_abstract_operations::TerminalAbstractOperation;
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    use super::*;

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(128, 128, 128, 128, 16).unwrap()
    }

    fn artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(2_001).unwrap();
        let entry = BlockId::new(2_002).unwrap();
        let exit = BlockId::new(2_003).unwrap();
        let left = ValueId::new(2_004).unwrap();
        let right = ValueId::new(2_005).unwrap();
        let computed = ValueId::new(2_006).unwrap();
        let forwarded = ValueId::new(2_007).unwrap();
        let also_forwarded = ValueId::new(2_016).unwrap();
        let result = ValueId::new(2_008).unwrap();
        let obligation = ObligationId::new(2_009).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![
                            Operation {
                                id: OperationId::new(2_010).unwrap(),
                                result: OperationResult::Scalar(declaration(left)),
                                kind: OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(7),
                                },
                            },
                            Operation {
                                id: OperationId::new(2_011).unwrap(),
                                result: OperationResult::Scalar(declaration(right)),
                                kind: OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(8),
                                },
                            },
                            Operation {
                                id: OperationId::new(2_012).unwrap(),
                                result: OperationResult::Scalar(declaration(computed)),
                                kind: OperationKind::ExactIntegerAdd {
                                    left,
                                    right,
                                    obligation,
                                },
                            },
                        ],
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(2_013).unwrap(),
                            target: exit,
                            arguments: vec![computed, computed],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: exit,
                        parameters: vec![declaration(forwarded), declaration(also_forwarded)],
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: EdgeId::new(2_014).unwrap(),
                            value: forwarded,
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(2_015).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn request(selections: OptimizationSelections) -> ExplicitOptimizationRequest {
        ExplicitOptimizationRequest::new(selections, budget()).unwrap()
    }

    #[test]
    fn empty_selection_cannot_enter_or_decode_artifacts() {
        assert_eq!(
            ExplicitOptimizationRequest::new(OptimizationSelections::default(), budget()),
            Err(EmptyOptimizationSelections)
        );
    }

    #[test]
    fn canonical_two_pass_suite_retains_each_manifest_and_one_ledger() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SparseConditionalConstantPropagation,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert_eq!(optimized.commits().len(), 3);
        assert_eq!(optimized.pass_manifests().len(), 2);
        assert_eq!(optimized.transformation_ledger().records().len(), 3);
        assert_eq!(
            optimized.pass_manifests()[0].output(),
            optimized.pass_manifests()[1].input()
        );
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(
            optimized.plan().functions[0].block_entries[1]
                .parameters
                .is_empty()
        );
        assert!(matches!(
            &optimized.plan().functions[0].operations[3],
            TerminalAbstractOperation::Jump { bindings, .. } if bindings.is_empty()
        ));
        assert!(matches!(
            &optimized.plan().functions[0].operations[4],
            TerminalAbstractOperation::Return { value, .. } if *value == ValueId::new(2_006).unwrap()
        ));
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        let staged = stage_optimized_assignment(target).unwrap();
        assert_eq!(staged.assigned().functions.len(), 1);
        assert_eq!(staged.custody().function_count(), 1);
        assert_eq!(
            staged.custody().optimization(),
            staged
                .optimized_target()
                .optimized()
                .identity_bundle()
                .identity()
        );
    }

    #[test]
    fn two_pass_artifact_orchestration_is_deterministic() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let run = || {
            optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(selections.clone()),
            )
            .unwrap()
        };
        let first = run();
        let second = run();

        assert_eq!(first.plan(), second.plan());
        assert_eq!(first.pass_manifests(), second.pass_manifests());
        assert_eq!(
            first.transformation_ledger(),
            second.transformation_ledger()
        );
        assert_eq!(first.identity_bundle(), second.identity_bundle());
        assert_eq!(first.validation(), second.validation());
    }

    #[test]
    fn unsupported_selection_fails_without_compatibility_fallback() {
        let (semantic, proof) = artifact();
        let error = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            OptimizationPipelineError::Run(OptimizationRunError::RegistryConstruction(
                RuleRegistryError::UnsupportedOptimization(Optimization::ControlFlowCleanup)
            ))
        ));
    }

    #[test]
    fn per_pass_budget_exhaustion_publishes_no_carrier() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
        let error = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, constrained).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            OptimizationPipelineError::Run(OptimizationRunError::WorkBudgetExhausted("commits"))
        ));
    }

    #[test]
    fn staged_assignment_is_deterministic_and_retains_optimizer_custody() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let stage = || {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(selections.clone()),
            )
            .unwrap();
            let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
                optimized,
                NativeTarget::linux_x64(),
            )
            .unwrap();
            stage_optimized_assignment(target).unwrap()
        };
        let first = stage();
        let second = stage();

        assert_eq!(first.assigned(), second.assigned());
        assert_eq!(first.custody(), second.custody());
        assert_eq!(
            first.optimized_target().optimized().transformation_ledger(),
            second
                .optimized_target()
                .optimized()
                .transformation_ledger()
        );
        assert_eq!(
            first.optimized_target().optimized().pass_manifests(),
            second.optimized_target().optimized().pass_manifests()
        );
    }

    #[test]
    fn independent_assignment_custody_rejects_each_root_and_provenance_corruption() {
        let (semantic, proof) = artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(
                OptimizationSelections::new([
                    Optimization::SparseConditionalConstantPropagation,
                    Optimization::CopyPropagation,
                ])
                .unwrap(),
            ),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        let staged = stage_optimized_assignment(target).unwrap();

        let mut corrupted = staged.assigned().clone();
        corrupted.terminal_psi.program_fingerprint =
            psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]);
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.target = NativeTarget::windows_x64();
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::NativeTargetMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.entry = MachineId::new(9_001).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::EntryMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions.push(corrupted.functions[0].clone());
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
                expected: 1,
                actual: 2,
            })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].machine = MachineId::new(9_002).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].attachment = Some(psi_core::StructuralTypeId::new(9_003).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0]
            .provenance
            .operations
            .push(OperationId::new(9_004).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(staged.optimized_target(), &corrupted),
            Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position: 0 })
        );
    }
}
