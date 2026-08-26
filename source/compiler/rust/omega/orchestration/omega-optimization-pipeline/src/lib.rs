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
mod register_environment;
mod selection;

pub use assignment::{
    OptimizedAssignmentCustodyError, OptimizedAssignmentPipelineError,
    StagedOptimizedAssignedOperations, StagedOptimizedAssignmentCustodyReceipt,
    stage_optimized_assignment, stage_optimized_assignment_with_provider_executions,
    validate_optimized_assignment_custody,
};
pub use register_environment::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment, validate_target_register_environment,
};
pub use selection::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    stage_optimized_instruction_selection, validate_optimized_selection_custody,
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
    use omega_optimization_unit::ValueDefinitionSite;
    use omega_psi_optimizer::{OptimizationRunError, RuleRegistryError};
    use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
    use omega_target::NativeTarget;
    use omega_terminal_abstract_operations::{TerminalAbstractOperation, TerminalValueBinding};
    use omega_terminal_selected_instructions::{
        TerminalSelectedInstructionKind, TerminalSelectedTerminator,
    };
    use omega_terminal_target_operations_to_selected_instructions::{
        SelectedInstructionError, terminal_selected_instruction_plan_identity,
        validate_terminal_selected_instructions,
    };
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
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

    fn conditional_immediate_artifact() -> (Vec<u8>, Vec<u8>) {
        conditional_immediate_artifact_with_type(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )
    }

    fn conditional_immediate_artifact_with_type(integer_type: IntegerType) -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(3_001).unwrap();
        let entry = BlockId::new(3_002).unwrap();
        let when_true = BlockId::new(3_003).unwrap();
        let when_false = BlockId::new(3_004).unwrap();
        let condition = ValueId::new(3_005).unwrap();
        let true_value = ValueId::new(3_006).unwrap();
        let false_value = ValueId::new(3_007).unwrap();
        let result = ValueId::new(3_008).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
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
                parameters: vec![declaration(condition, ScalarType::Boolean)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
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
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(3_011).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(3_012).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(3_009).unwrap(),
                            result: OperationResult::Scalar(declaration(true_value, scalar_type)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        }],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(3_013).unwrap(),
                            value: true_value,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(3_010).unwrap(),
                            result: OperationResult::Scalar(declaration(false_value, scalar_type)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(9),
                            },
                        }],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(3_014).unwrap(),
                            value: false_value,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(3_015).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_immediate_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
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
        assert_eq!(
            staged.register_environment().target(),
            NativeTarget::linux_x64()
        );
        assert_eq!(
            staged
                .register_environment()
                .physical()
                .model()
                .architecture,
            omega_target::Architecture::X86_64
        );
        assert_eq!(staged.custody().function_count(), 1);
        assert_eq!(
            staged.custody().optimization(),
            staged
                .optimized_target()
                .optimized()
                .identity_bundle()
                .identity()
        );
        assert_eq!(
            staged.custody().projection(),
            staged
                .optimized_target()
                .optimized()
                .validation()
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
    fn unsupported_selected_instruction_source_shape_fails_at_selection_boundary() {
        let (semantic, proof) = artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_instruction_selection(target),
            Err(OptimizedSelectionPipelineError::Selection(
                SelectedInstructionError::UnsupportedSourceShape { function: 0 }
            ))
        ));
    }

    #[test]
    fn non_u64_conditional_fails_at_named_integer_selection_boundary() {
        let (semantic, proof) = conditional_immediate_artifact_with_type(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        );
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_instruction_selection(target),
            Err(OptimizedSelectionPipelineError::Selection(
                SelectedInstructionError::UnsupportedIntegerShape { function: 0 }
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

        let wrong_environment =
            baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                &wrong_environment,
                staged.assigned(),
            ),
            Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.terminal_psi.program_fingerprint =
            psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]);
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.target = NativeTarget::windows_x64();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::NativeTargetMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.entry = MachineId::new(9_001).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::EntryMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions.push(corrupted.functions[0].clone());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
                expected: 1,
                actual: 2,
            })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].machine = MachineId::new(9_002).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].attachment = Some(psi_core::StructuralTypeId::new(9_003).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0]
            .provenance
            .operations
            .push(OperationId::new(9_004).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position: 0 })
        );
    }

    #[test]
    fn verified_three_block_conditional_selects_typed_vregs_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_conditional(target);
            let plan = staged.selected().plan();
            assert_eq!(plan.functions.len(), 1);
            assert_eq!(plan.functions[0].blocks.len(), 3);
            assert_eq!(plan.functions[0].virtual_registers.len(), 3);
            assert_eq!(staged.selected().receipt().instruction_count(), 6);
            assert_eq!(
                staged.custody().optimization_unit(),
                staged.optimized_target().optimized().unit().identity
            );
            assert_eq!(staged.custody().fuel_schedule(), plan.fuel_schedule);
            assert_eq!(
                staged.custody().selected(),
                staged.selected().receipt().identity()
            );

            let entry = &plan.functions[0].blocks[0];
            assert_eq!(
                entry.instructions[0].kind,
                TerminalSelectedInstructionKind::CompareI64Zero
            );
            assert!(entry.instructions[0].provenance.fuel.is_empty());
            let TerminalSelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            } = &entry.terminator
            else {
                panic!("entry must branch")
            };
            assert_eq!(
                instruction.kind,
                TerminalSelectedInstructionKind::ConditionalBranchNonZero
            );
            assert!(instruction.provenance.fuel.is_empty());
            assert_eq!(when_nonzero.fuel.len(), 1);
            assert_eq!(when_zero.fuel.len(), 1);
            assert_ne!(when_nonzero.psi_edge, when_zero.psi_edge);
            for block in &plan.functions[0].blocks[1..] {
                assert!(matches!(
                    block.instructions[0].kind,
                    TerminalSelectedInstructionKind::MaterializeI64 { .. }
                ));
                assert_eq!(block.instructions[0].provenance.operations.len(), 1);
                assert_eq!(block.instructions[0].provenance.fuel.len(), 1);
                let TerminalSelectedTerminator::Return { instruction, .. } = &block.terminator
                else {
                    panic!("leaf must return")
                };
                assert!(instruction.operands[0].fixed_view.is_some());
                assert_eq!(instruction.provenance.fuel.len(), 1);
            }
        }
    }

    fn validate_raw_selection(
        staged: &StagedOptimizedSelectedInstructions,
        raw: omega_terminal_selected_instructions::TerminalSelectedInstructionPlan,
    ) -> Result<
        omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions,
        SelectedInstructionError,
    >{
        let constraints = crate::selection::selection_constraints(
            staged.optimized_target(),
            staged.register_environment(),
        );
        validate_terminal_selected_instructions(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            &constraints,
            staged.register_environment().physical(),
            staged.register_environment().constraints(),
            raw,
        )
    }

    #[test]
    fn selected_cfg_validator_rejects_target_state_path_and_value_corruption() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_conditional(target);

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[0].instructions[0]
                .implicit_defs
                .clear();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::ConstraintEffectMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } = &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            std::mem::swap(when_nonzero, when_zero);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].virtual_registers[0].entry_fixed_view = None;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::Return { instruction, .. } =
                &mut corrupted.functions[0].blocks[1].terminator
            else {
                unreachable!()
            };
            instruction.operands[0].fixed_view = None;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::Return { instruction, .. } =
                &mut corrupted.functions[0].blocks[1].terminator
            else {
                unreachable!()
            };
            instruction.operands[0].virtual_register =
                omega_terminal_selected_instructions::TerminalVirtualRegisterId(2);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::UseBeforeDefinition { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].kind =
                TerminalSelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(11),
                };
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0]
                .provenance
                .values[0] = ValueId::new(8_001).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            when_nonzero.psi_edge = EdgeId::new(8_002).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch { when_zero, .. } =
                &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            when_zero.fuel[0].units += 1;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
            ));
        }
    }

    #[test]
    fn selected_content_identity_binds_every_retained_field_class() {
        let staged = staged_conditional(NativeTarget::linux_x64());
        let original = staged.selected().plan();
        let identity = terminal_selected_instruction_plan_identity(original);
        let mut mutations = Vec::new();

        let mut changed = original.clone();
        changed.target = NativeTarget::windows_x64();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.entry = MachineId::new(8_009).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].machine = MachineId::new(8_018).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].attachment = Some(psi_core::StructuralTypeId::new(8_010).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0]
            .provenance
            .operations
            .push(OperationId::new(8_011).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0]
            .provenance
            .edges
            .push(EdgeId::new(8_019).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_block.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].scalar_type = ScalarType::Boolean;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].origin =
            omega_terminal_selected_instructions::TerminalVirtualRegisterOrigin::InstructionResult {
                instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(4),
                source_value: ValueId::new(8_012).unwrap(),
            };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].definition_site = ValueDefinitionSite::Node {
            block: BlockId::new(8_013).unwrap(),
            node: 7,
        };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].entry_fixed_view = None;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].source_block = BlockId::new(8_020).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions.clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].kind =
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(12),
            };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .constraint
            .variant += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].access =
            RegisterOperandAccess::Use;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .implicit_uses
            .push(RegisterUnitId(999));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .implicit_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .clobbers
            .push(RegisterUnitId(998));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .operations
            .push(OperationId::new(8_021).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .values
            .push(ValueId::new(8_022).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .edges
            .push(EdgeId::new(8_023).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .obligations
            .push(ObligationId::new(8_014).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .fuel[0]
            .units += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.bindings.push(TerminalValueBinding {
            parameter: ValueId::new(8_015).unwrap(),
            argument: ValueId::new(8_016).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.source_target = BlockId::new(8_024).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_zero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_zero.fuel[0].units += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::Return {
            psi_return_edge, ..
        } = &mut changed.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        *psi_return_edge = EdgeId::new(8_017).unwrap();
        mutations.push(changed);

        for mutation in mutations {
            assert_ne!(
                terminal_selected_instruction_plan_identity(&mutation),
                identity
            );
        }
    }

    #[test]
    fn staged_selection_custody_rejects_detached_environment_and_selected_plan() {
        let x86 = staged_conditional(NativeTarget::linux_x64());
        let arm = staged_conditional(NativeTarget::linux_arm64());
        assert_eq!(
            validate_optimized_selection_custody(
                x86.optimized_target(),
                arm.register_environment(),
                x86.selected(),
            ),
            Err(OptimizedSelectionCustodyError::RegisterEnvironmentTargetMismatch)
        );
        assert_eq!(
            validate_optimized_selection_custody(
                x86.optimized_target(),
                x86.register_environment(),
                arm.selected(),
            ),
            Err(OptimizedSelectionCustodyError::RootMismatch)
        );

        let mut target = x86.optimized_target().target_operations().clone();
        let forged_operation = OperationId::new(8_030).unwrap();
        target.functions[0]
            .provenance
            .operations
            .push(forged_operation);
        let mut selected = x86.selected().plan().clone();
        selected.functions[0]
            .provenance
            .operations
            .push(forged_operation);
        let constraints = crate::selection::selection_constraints(
            x86.optimized_target(),
            x86.register_environment(),
        );
        assert_eq!(
            validate_terminal_selected_instructions(
                &target,
                x86.optimized_target().optimized().plan(),
                x86.optimized_target().optimized().unit(),
                &constraints,
                x86.register_environment().physical(),
                x86.register_environment().constraints(),
                selected,
            ),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        );

        let mut unit = x86.optimized_target().optimized().unit().clone();
        unit.functions[0].blocks[0].nodes[0].effect.output += 1_000;
        unit.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&unit);
        assert_eq!(
            validate_terminal_selected_instructions(
                x86.optimized_target().target_operations(),
                x86.optimized_target().optimized().plan(),
                &unit,
                &constraints,
                x86.register_environment().physical(),
                x86.register_environment().constraints(),
                x86.selected().plan().clone(),
            ),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        );
    }
}
