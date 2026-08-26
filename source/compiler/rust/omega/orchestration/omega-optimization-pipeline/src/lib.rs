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

mod allocation_legality;
mod assignment;
mod fixed_view_copies;
mod function_relative_realization;
mod literal_fold_homes;
mod literal_folds;
mod live_ranges;
mod liveness;
mod machine_effects;
mod physical_pipeline;
mod post_allocation_machine_effects;
mod post_allocation_selected_form_encoding;
mod register_environment;
mod register_homes;
mod resolved_selected_form_layout;
mod selected_reanalysis;
mod selection;
mod whole_function_exit_contract;

pub use allocation_legality::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_allocation_legality_with_availability,
    validate_optimized_allocation_legality_custody,
};
pub use assignment::{
    OptimizedAssignmentCustodyError, OptimizedAssignmentPipelineError,
    StagedOptimizedAssignedOperations, StagedOptimizedAssignmentCustodyReceipt,
    stage_optimized_assignment, stage_optimized_assignment_with_provider_executions,
    validate_optimized_assignment_custody,
};
pub use fixed_view_copies::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, stage_optimized_fixed_view_copies,
    validate_optimized_fixed_view_copy_custody,
};
pub use function_relative_realization::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationRealizationStatistics, FunctionRelativeOptimizationUnavailableData,
    StagedSelectedLoweringFunctionRelativeRealization,
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    stage_selected_lowering_function_relative_realization,
    validate_selected_lowering_function_relative_realization_custody,
};
pub use literal_fold_homes::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_selected_lowering,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};
pub use literal_folds::{
    OptimizedLiteralFoldCustodyError, SelectedLoweringOptimizationSchedule,
    StagedOptimizedLiteralFoldAttempt, StagedOptimizedLiteralFoldAttemptReceipt,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedLiteralFolds,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    run_selected_lowering_optimizations, stage_first_optimized_literal_fold,
    stage_next_optimized_literal_fold, validate_optimized_literal_fold_custody,
    validate_selected_lowering_optimization_custody,
};
pub use live_ranges::{
    OptimizedLiveRangeCustodyError, StagedOptimizedLiveRangeCustodyReceipt,
    StagedOptimizedLiveRanges, stage_optimized_live_ranges, validate_optimized_live_range_custody,
};
pub use liveness::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
    stage_optimized_liveness, validate_optimized_liveness_custody,
};
pub use machine_effects::{
    OptimizedMachineEffectPipelineError, StagedOptimizedMachineEffectCustodyReceipt,
    StagedOptimizedMachineEffectSourceCustodyReceipt, StagedOptimizedMachineEffects,
    stage_optimized_machine_effects, stage_optimized_machine_effects_after_fixed_view_copies,
    stage_optimized_machine_effects_after_literal_folds,
    stage_optimized_machine_effects_after_selected_lowering,
    validate_optimized_machine_effect_custody,
    validate_optimized_machine_effect_custody_after_fixed_view_copies,
    validate_optimized_machine_effect_custody_after_literal_folds,
    validate_optimized_machine_effect_custody_after_selected_lowering,
};
pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
};
pub use post_allocation_machine_effects::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan_after_literal_folds,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody,
    validate_optimized_post_allocation_machine_plan_after_literal_fold_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};
pub use post_allocation_selected_form_encoding::{
    DeferredTerminalControlEncodingReason, OptimizedSelectedFormEncodingError,
    StagedOptimizedSelectedFormEncoding, TerminalSelectedFormDecodedFootprint,
    TerminalSelectedFormEncodingIdentity, TerminalSelectedFormEncodingRow,
    TerminalSelectedFormEncodingState, stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
pub use register_environment::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment, validate_target_register_environment,
    validate_target_register_environment_with_reservations,
};
pub use register_homes::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};
pub use resolved_selected_form_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    TerminalResolvedConditionalBranchEvidence, TerminalResolvedSelectedBlockLayout,
    TerminalResolvedSelectedFormLayoutIdentity, TerminalResolvedSelectedFormRow,
    TerminalResolvedSelectedFunctionLayout, TerminalSelectedFunctionLayoutPolicy,
    stage_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout,
};
pub use selected_reanalysis::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, stage_optimized_selected_reanalysis,
    validate_optimized_selected_reanalysis_custody,
};
pub use selection::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    stage_optimized_instruction_selection, validate_optimized_selection_custody,
};
pub use whole_function_exit_contract::{
    TerminalWholeFunctionEntryAssumption, TerminalWholeFunctionExitContract,
    TerminalWholeFunctionExitContractError, TerminalWholeFunctionExitContractIdentity,
    TerminalWholeFunctionExitEvidence, TerminalWholeFunctionExitPolicy,
    TerminalWholeFunctionHardeningPolicy, TerminalWholeFunctionReturnEvidence,
    TerminalWholeFunctionReturnMechanism, ValidatedTerminalWholeFunctionExitContract,
    stage_terminal_whole_function_exit_contract, validate_terminal_whole_function_exit_contract,
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
    use std::collections::BTreeSet;

    use omega_optimization_core::{
        Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use omega_optimization_unit::ValueDefinitionSite;
    use omega_psi_optimizer::{OptimizationRunError, RuleRegistryError};
    use omega_regalloc::{
        PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
        PostAllocationSelectedTransformation, TerminalAllocationLegalityError,
        TerminalAllocatorAvailabilityError, TerminalAllocatorAvailabilityPolicy,
        TerminalArchitecturalUnitActionKind, TerminalFixedViewCopyError,
        TerminalFixedViewCopyPolicy, TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy,
        TerminalLiveRangeError, TerminalLiveRangeFragment, TerminalLiveRangePoint,
        TerminalLivenessError, TerminalRecoveryClassification,
        TerminalRecoveryClassificationPolicy, TerminalRecoveryVictimRole,
        TerminalRegisterHomeError, TerminalRegisterHomePlan, TerminalSpillChoicePolicy,
        TerminalVirtualFixedConstraintSite, TerminalVirtualInterference,
        analyze_terminal_allocation_legality, analyze_terminal_live_ranges,
        analyze_terminal_liveness, choose_terminal_spill_victims,
        classify_terminal_pressure_recovery, fold_terminal_selected_incoming_literal,
        materialize_terminal_allocator_availability, terminal_allocation_legality_identity,
        terminal_fixed_view_copy_identity, terminal_live_range_identity,
        terminal_liveness_identity, terminal_register_home_identity,
        validate_post_allocation_optimization_manifest, validate_terminal_allocation_legality,
        validate_terminal_allocator_availability, validate_terminal_fixed_view_copies,
        validate_terminal_literal_fold, validate_terminal_live_ranges, validate_terminal_liveness,
        validate_terminal_register_homes,
    };
    use omega_register_model::{
        RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterViewId,
        target_register_environment_identity, validate_register_reservation_profile,
    };
    use omega_target::NativeTarget;
    use omega_terminal_abstract_operations::{TerminalAbstractOperation, TerminalValueBinding};
    use omega_terminal_selected_instructions::{
        TerminalSelectedInstructionKind, TerminalSelectedTerminator, TerminalVirtualRegisterId,
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

    fn selected_lowering_budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 64).unwrap()
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

    fn conditional_forwarded_parameter_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_001).unwrap();
        let entry = BlockId::new(4_002).unwrap();
        let when_true = BlockId::new(4_003).unwrap();
        let when_false = BlockId::new(4_004).unwrap();
        let condition = ValueId::new(4_005).unwrap();
        let forwarded = ValueId::new(4_006).unwrap();
        let result = ValueId::new(4_007).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
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
                parameters: vec![
                    declaration(condition, ScalarType::Boolean),
                    declaration(forwarded, scalar_type),
                ],
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
                                edge: EdgeId::new(4_011).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(4_012).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_013).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_014).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_015).unwrap(),
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

    fn staged_forwarded_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
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

    fn conditional_exact_binary_artifact(subtract: bool) -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(5_001).unwrap();
        let entry = BlockId::new(5_002).unwrap();
        let when_true = BlockId::new(5_003).unwrap();
        let when_false = BlockId::new(5_004).unwrap();
        let condition = ValueId::new(5_005).unwrap();
        let true_left = ValueId::new(5_006).unwrap();
        let true_right = ValueId::new(5_007).unwrap();
        let true_sum = ValueId::new(5_008).unwrap();
        let false_left = ValueId::new(5_009).unwrap();
        let false_right = ValueId::new(5_010).unwrap();
        let false_sum = ValueId::new(5_011).unwrap();
        let result = ValueId::new(5_012).unwrap();
        let true_left_operation = OperationId::new(5_021).unwrap();
        let true_right_operation = OperationId::new(5_022).unwrap();
        let true_add_operation = OperationId::new(5_023).unwrap();
        let false_left_operation = OperationId::new(5_024).unwrap();
        let false_right_operation = OperationId::new(5_025).unwrap();
        let false_add_operation = OperationId::new(5_026).unwrap();
        let true_obligation = ObligationId::new(5_031).unwrap();
        let false_obligation = ObligationId::new(5_032).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let integer_operation = |id, result, kind| Operation {
            id,
            result: OperationResult::Scalar(declaration(result, scalar_type)),
            kind,
        };
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
                                edge: EdgeId::new(5_041).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(5_042).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: vec![
                            integer_operation(
                                true_left_operation,
                                true_left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 13 } else { 7 }),
                                },
                            ),
                            integer_operation(
                                true_right_operation,
                                true_right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 5 } else { 8 }),
                                },
                            ),
                            integer_operation(
                                true_add_operation,
                                true_sum,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_043).unwrap(),
                            value: true_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: vec![
                            integer_operation(
                                false_left_operation,
                                false_left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 21 } else { 11 }),
                                },
                            ),
                            integer_operation(
                                false_right_operation,
                                false_right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 8 } else { 13 }),
                                },
                            ),
                            integer_operation(
                                false_add_operation,
                                false_sum,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_044).unwrap(),
                            value: false_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(5_051).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [true_obligation, false_obligation]
                .into_iter()
                .map(|obligation| ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                })
                .collect(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_exact_add_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        staged_exact_add_conditional_with_selections(
            target,
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            budget(),
        )
    }

    fn staged_exact_add_conditional_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, budget).unwrap(),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn staged_exact_subtract_conditional(
        target: NativeTarget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_exact_binary_artifact(true);
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
            staged.custody().register_environment(),
            staged.register_environment().identity()
        );
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
        assert_eq!(
            staged.custody().manifest(),
            staged
                .optimized_target()
                .optimized()
                .pre_physical_manifest()
                .record()
                .identity
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
    fn mixed_phase_suite_retains_the_full_request_and_exact_psi_projection() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
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
        assert_eq!(
            optimized.psi_selections().as_slice(),
            &[Optimization::SparseConditionalConstantPropagation]
        );
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(
            optimized.pre_physical_manifest().record().selections,
            selections
        );
    }

    #[test]
    fn lower_only_suite_reaches_prephysical_custody_without_claiming_psi_work() {
        let (semantic, proof) = artifact();
        let selections =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert!(optimized.psi_selections().is_empty());
        assert!(optimized.commits().is_empty());
        assert!(optimized.pass_manifests().is_empty());
        assert!(
            optimized
                .pre_physical_manifest()
                .record()
                .psi_selections
                .is_empty()
        );
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
                staged.custody().register_environment(),
                staged.register_environment().identity()
            );
            assert_eq!(
                staged.custody().selected(),
                staged.selected().receipt().identity()
            );
            let mut copy_tagged = plan.clone();
            copy_tagged.functions[0].blocks[1].instructions[0].kind =
                TerminalSelectedInstructionKind::CopyI64;
            assert_ne!(
                terminal_selected_instruction_plan_identity(&copy_tagged),
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

    #[test]
    fn exact_add_selection_retains_proof_policy_and_target_constraints() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_exact_add_conditional(target);
            let plan = staged.selected().plan();
            let function = &plan.functions[0];
            assert_eq!(function.virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            assert_eq!(accepted.len(), 2);
            for (block, expected_obligation) in function.blocks[1..].iter().zip([
                ObligationId::new(5_031).unwrap(),
                ObligationId::new(5_032).unwrap(),
            ]) {
                assert_eq!(block.instructions.len(), 3);
                let add = &block.instructions[2];
                let TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                } = add.kind
                else {
                    panic!("leaf arithmetic must retain exact-add semantics")
                };
                assert_eq!(obligation, expected_obligation);
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == accepted_fact)
                    .expect("selected fact must remain verifier-owned");
                assert_eq!(fact.operation, add.provenance.operations[0]);
                assert_eq!(fact.obligation, obligation);
                assert_eq!(
                    add.constraint,
                    staged.register_environment().selected_keys().add_i64
                );
                assert_eq!(
                    add.operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect::<Vec<_>>(),
                    vec![
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Def,
                    ]
                );
                assert!(
                    add.operands
                        .iter()
                        .all(|operand| operand.fixed_view.is_none())
                );
                assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
                assert!(add.implicit_uses.is_empty());
                assert!(add.implicit_defs.is_empty());
                assert!(add.clobbers.is_empty());
                assert_eq!(add.provenance.operations.len(), 1);
                assert_eq!(add.provenance.values.len(), 3);
                assert_eq!(add.provenance.obligations, vec![obligation]);
                assert_eq!(add.provenance.fuel.len(), 1);
            }

            let original_identity = staged.selected().receipt().identity();
            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactAddI64 { obligation, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(9_501).unwrap();
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                original_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            let false_fact = match corrupted.functions[0].blocks[2].instructions[2].kind {
                TerminalSelectedInstructionKind::ExactAddI64 { accepted_fact, .. } => accepted_fact,
                _ => unreachable!(),
            };
            let TerminalSelectedInstructionKind::ExactAddI64 { accepted_fact, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *accepted_fact = false_fact;
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                original_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .obligations[0] = ObligationId::new(9_502).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .operands
                .swap(0, 1);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2].constraint =
                staged.register_environment().selected_keys().copy_i64;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .operations[0] = OperationId::new(9_503).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .fuel[0]
                .units += 1;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
            ));
        }
    }

    #[test]
    fn exact_subtract_retains_proof_target_effects_and_reaches_homes() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_exact_subtract_conditional(target);
            let plan = staged.selected().plan();
            assert_eq!(plan.functions[0].virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            for (block, expected_obligation) in plan.functions[0].blocks[1..].iter().zip([
                ObligationId::new(5_031).unwrap(),
                ObligationId::new(5_032).unwrap(),
            ]) {
                let subtract = &block.instructions[2];
                let TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation,
                    accepted_fact,
                } = subtract.kind
                else {
                    panic!("leaf arithmetic must retain exact-subtract semantics")
                };
                assert_eq!(obligation, expected_obligation);
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == accepted_fact)
                    .expect("selected fact must remain verifier-owned");
                assert_eq!(fact.operation, subtract.provenance.operations[0]);
                assert_eq!(fact.obligation, obligation);
                assert_eq!(
                    subtract.constraint,
                    staged.register_environment().selected_keys().subtract_i64
                );
                assert_eq!(
                    subtract
                        .operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect::<Vec<_>>(),
                    vec![
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Def,
                    ]
                );
                assert!(subtract.implicit_uses.is_empty());
                assert!(subtract.implicit_defs.is_empty());
                if target.architecture == omega_target::Architecture::X86_64 {
                    assert!(!subtract.clobbers.is_empty());
                } else {
                    assert!(subtract.clobbers.is_empty());
                }
                assert_eq!(subtract.provenance.obligations, vec![obligation]);
                assert_eq!(subtract.provenance.fuel.len(), 1);
            }

            let identity = staged.selected().receipt().identity();
            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactSubtractI64 { obligation, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(9_504).unwrap();
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactSubtractI64 {
                obligation,
                accepted_fact,
            } = corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            corrupted.functions[0].blocks[1].instructions[2].kind =
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                };
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let homes = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged).expect("subtract liveness"),
                    )
                    .expect("subtract ranges"),
                )
                .expect("subtract legality"),
            )
            .expect("subtract homes");
            assert_eq!(homes.custody().assignment_count(), 7);
            let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
            assert_eq!(post.custody().instruction_count(), 10);
            assert_eq!(
                post.custody().source(),
                &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(
                    homes.custody()
                )
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_custody(&homes, &post).unwrap(),
                post.custody()
            );
            let selected_stage = homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let encodings = stage_optimized_layout_independent_selected_form_encoding(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
            )
            .unwrap();
            assert_eq!(encodings.selected(), post.machine().receipt().selected());
            assert_eq!(encodings.machine(), post.machine().receipt().identity());
            assert_eq!(encodings.rows().len(), 10);
            assert_eq!(
                encodings
                    .rows()
                    .iter()
                    .filter(|row| matches!(
                        row.state,
                        TerminalSelectedFormEncodingState::DeferredControl { .. }
                    ))
                    .count(),
                1
            );
            assert!(encodings.rows().iter().all(|row| match &row.state {
                TerminalSelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
                TerminalSelectedFormEncodingState::DeferredControl { .. } => true,
            }));
            let returns = encodings
                .rows()
                .iter()
                .filter(|row| {
                    row.alternative.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ReturnI64
                })
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 2);
            for returned in returns {
                let TerminalSelectedFormEncodingState::Encoded { bytes, footprint } =
                    &returned.state
                else {
                    panic!("returns have layout-independent target encodings")
                };
                assert_eq!(
                    bytes.as_slice(),
                    if target.architecture == omega_target::Architecture::X86_64 {
                        &[0xc3][..]
                    } else {
                        &[0xc0, 0x03, 0x5f, 0xd6][..]
                    }
                );
                assert!(footprint.register_reads.is_empty());
                assert!(footprint.register_writes.is_empty());
                assert!(footprint.encoded.external_operand_reads.is_empty());
                assert!(footprint.encoded.external_operand_writes.is_empty());
            }
            validate_optimized_layout_independent_selected_form_encoding(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
            )
            .unwrap();
            let layout = stage_optimized_resolved_selected_form_layout(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
            )
            .unwrap();
            assert_eq!(
                layout.policy(),
                TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
            );
            assert_eq!(layout.pre_layout(), encodings.identity());
            assert_eq!(layout.functions().len(), 1);
            let selected_function = &selected_stage.selected().plan().functions[0];
            let TerminalSelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } = &selected_function
                .blocks
                .iter()
                .find(|block| block.id == selected_function.entry_block)
                .unwrap()
                .terminator
            else {
                panic!("fixture entry is conditional")
            };
            let function_layout = &layout.functions()[0];
            assert_eq!(
                function_layout
                    .blocks
                    .iter()
                    .map(|block| block.block)
                    .collect::<Vec<_>>(),
                [
                    selected_function.entry_block,
                    when_zero.block,
                    when_nonzero.block
                ]
            );
            assert!(
                function_layout
                    .blocks
                    .windows(2)
                    .all(|pair| pair[0].offset + pair[0].byte_count == pair[1].offset)
            );
            let branch = function_layout
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find_map(|row| row.branch.as_deref().map(|branch| (row, branch)))
                .expect("one resolved branch");
            assert_eq!(branch.1.when_zero_block, when_zero.block);
            assert_eq!(branch.1.when_nonzero_block, when_nonzero.block);
            assert_eq!(
                branch.0.offset + u64::try_from(branch.0.bytes.len()).unwrap(),
                branch.1.when_zero_offset
            );
            match target.architecture {
                omega_target::Architecture::X86_64 => {
                    assert_eq!(&branch.0.bytes[..2], [0x0f, 0x85]);
                    assert_eq!(
                        branch.1.byte_displacement,
                        i64::try_from(branch.1.when_nonzero_offset).unwrap()
                            - i64::try_from(branch.0.offset + 6).unwrap()
                    );
                }
                omega_target::Architecture::Aarch64 => {
                    assert_eq!(branch.0.bytes[0] & 0x1f, 1);
                    assert_eq!(
                        branch.1.byte_displacement,
                        i64::try_from(branch.1.when_nonzero_offset).unwrap()
                            - i64::try_from(branch.0.offset).unwrap()
                    );
                }
            }
            validate_optimized_resolved_selected_form_layout(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
                &layout,
            )
            .unwrap();
            let mut corrupted_layout = layout.clone();
            corrupted_layout.functions_mut()[0].blocks[0]
                .instructions
                .last_mut()
                .unwrap()
                .bytes[0] ^= 1;
            assert_eq!(
                validate_optimized_resolved_selected_form_layout(
                    selected_stage.selected(),
                    &post,
                    selected_stage.register_environment().physical(),
                    &encodings,
                    &corrupted_layout,
                ),
                Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
            );
            let subtracts = post
                .machine()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    instruction.alternative.key.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ExactSubtractI64
                })
                .collect::<Vec<_>>();
            assert_eq!(subtracts.len(), 2);
            assert!(subtracts.iter().all(|instruction| {
                instruction.operands.len() == 3
                    && instruction
                        .unit_uses
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && instruction
                        .unit_defs
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && instruction
                        .operands
                        .iter()
                        .filter(|operand| operand.write_semantics.is_some())
                        .all(|operand| !operand.write_units.is_empty())
            }));
            let mut corrupted = post.machine().plan().clone();
            let subtract = corrupted.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    instruction.alternative.key.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ExactSubtractI64
                })
                .unwrap();
            subtract.alternative.key.variant = u32::MAX;
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert!(matches!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::InstructionMismatch {
                        ..
                    }
                )
            ));

            let mut corrupted = post.machine().plan().clone();
            corrupted.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .flat_map(|instruction| &mut instruction.operands)
                .find(|operand| operand.write_semantics.is_some())
                .unwrap()
                .write_units
                .clear();
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert!(matches!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::InstructionMismatch {
                        ..
                    }
                )
            ));

            let mut corrupted = post.machine().plan().clone();
            corrupted.effects =
                omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity::from_bytes(
                    [0x5a; 32],
                );
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert_eq!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::EffectRootMismatch
                )
            );
        }
    }

    #[test]
    fn machine_effect_sidecar_reconstructs_subtraction_and_control_barriers() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_exact_subtract_conditional(target);
            let staged = stage_optimized_machine_effects(&selected).unwrap();
            assert_eq!(staged.custody().instruction_count(), 10);
            assert_eq!(
                staged.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(selected.custody())
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody(&selected, staged.effects()).unwrap(),
                staged.custody()
            );
            let instructions = staged
                .effects()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .collect::<Vec<_>>();
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| {
                        instruction.barrier
                        == omega_terminal_selected_instructions::TerminalMachineBarrier::ControlFlow
                    })
                    .count(),
                3
            );
            let subtracts = instructions
                .iter()
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        TerminalSelectedInstructionKind::ExactSubtractI64 { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(subtracts.len(), 2);
            for subtract in subtracts {
                assert_eq!(
                    subtract.alternatives.len(),
                    if target.architecture == omega_target::Architecture::X86_64 {
                        4
                    } else {
                        1
                    }
                );
                assert_eq!(
                    subtract.unit_clobbers.is_empty(),
                    target.architecture != omega_target::Architecture::X86_64
                );
                assert_eq!(subtract.provenance.obligations.len(), 1);
                assert_eq!(subtract.provenance.fuel.len(), 1);
            }

            let mut corrupted = staged.effects().plan().clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .alternatives
                .clear();
            assert!(matches!(
                omega_machine_optimizer::validate_terminal_pre_allocation_machine_effects(
                    selected.selected(),
                    selected.register_environment().identity(),
                    selected.register_environment().physical(),
                    selected.register_environment().constraints(),
                    selected.register_environment().reservations(),
                    selected.register_environment().allocation_constraint_keys(),
                    &match target.architecture {
                        omega_target::Architecture::X86_64 => {
                            omega_terminal_isa_x86_64::validate_x86_64_terminal_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                                omega_terminal_isa_x86_64::x86_64_terminal_machine_effect_catalog(
                                    target,
                                    selected.register_environment().constraints(),
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        }
                        omega_target::Architecture::Aarch64 => {
                            omega_terminal_isa_aarch64::validate_aarch64_terminal_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                                omega_terminal_isa_aarch64::aarch64_terminal_machine_effect_catalog(
                                    target,
                                    selected.register_environment().constraints(),
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        }
                    },
                    corrupted,
                ),
                Err(
                    omega_machine_optimizer::TerminalMachineEffectError::InstructionMismatch { .. }
                )
            ));
        }
    }

    #[test]
    fn exact_add_pressure_reaches_deterministic_homes_on_both_architectures() {
        for (target, expected_homes) in [
            (
                NativeTarget::linux_x64(),
                ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
            ),
            (
                NativeTarget::linux_arm64(),
                ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
            ),
        ] {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            assert!(
                choices
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.choice.is_none())
            );
            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            assert!(
                recovery
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.classification.is_none())
            );
            assert_eq!(recovery.receipt().selected(), selected.custody().selected());
            assert_eq!(recovery.receipt().ranges(), ranges.custody().ranges());
            assert_eq!(recovery.receipt().legality(), legality.custody().legality());
            assert_eq!(
                recovery.receipt().spill_choices(),
                choices.receipt().identity()
            );
            let staged = stage_optimized_register_homes(legality).unwrap();
            let post = stage_optimized_post_allocation_machine_plan(&staged).unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                staged.custody().selected()
            );
            assert!(post.machine().plan().functions.iter().all(|function| {
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .all(|instruction| instruction.alternative.key.variant == 0)
            }));
            let legality_stage = staged.legality_stage();
            let ranges_stage = legality_stage.live_range_stage();
            let liveness_stage = ranges_stage.liveness_stage();
            let liveness = &liveness_stage.liveness().plan().functions[0];
            for (block, registers) in liveness.blocks[1..].iter().zip([[1_u32, 2, 3], [4, 5, 6]]) {
                assert_eq!(block.instructions.len(), 4);
                assert_eq!(
                    block.instructions[2].virtual_uses,
                    registers[..2]
                        .iter()
                        .copied()
                        .map(TerminalVirtualRegisterId)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    block.instructions[2].virtual_defs,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
                assert_eq!(
                    block.instructions[2].virtual_live_out,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
            }

            let ranges = &ranges_stage.ranges().plan().functions[0];
            assert_eq!(
                ranges
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 12), (2, 12, 20)]
            );
            assert_eq!(
                ranges.interference,
                vec![
                    TerminalVirtualInterference {
                        lower: TerminalVirtualRegisterId(1),
                        higher: TerminalVirtualRegisterId(2),
                    },
                    TerminalVirtualInterference {
                        lower: TerminalVirtualRegisterId(4),
                        higher: TerminalVirtualRegisterId(5),
                    },
                ]
            );
            assert!(
                ranges
                    .virtual_registers
                    .iter()
                    .all(|register| register.edge_connectors.is_empty())
            );
            assert_eq!(legality_stage.custody().entry_transition_count(), 0);

            let environment = liveness_stage.selected_stage().register_environment();
            let model = environment.physical().model();
            let homes = &staged.homes().plan().functions[0];
            assert_eq!(homes.assignments.len(), 7);
            assert_eq!(
                homes
                    .assignments
                    .iter()
                    .map(|assignment| {
                        model
                            .views
                            .iter()
                            .find(|view| view.id == assignment.view)
                            .unwrap()
                            .name
                            .as_str()
                    })
                    .collect::<Vec<_>>(),
                expected_homes
            );
            assert_eq!(homes.assignments[1].view, homes.assignments[4].view);
            assert_eq!(homes.assignments[2].view, homes.assignments[5].view);
            assert_ne!(homes.assignments[1].view, homes.assignments[2].view);
        }
    }

    #[test]
    fn explicit_one_view_availability_reaches_real_pressure_and_recovery_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rdi"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let fixed_return = (target == NativeTarget::linux_x64())
                .then(|| environment.physical().model().view_named("rax").unwrap().id);
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![sole_view, sole_view],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::NonCanonicalAllowlist)
            ));
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![RegisterViewId(u16::MAX)],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::UnknownView { .. })
            ));
            let reserved_view =
                environment
                    .physical()
                    .model()
                    .views
                    .iter()
                    .find(|view| {
                        view.allocatable
                            && view.units.iter().chain(&view.write_units).any(|unit| {
                                environment.reservations().reserved_units().contains(unit)
                            })
                    })
                    .unwrap();
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![reserved_view.id],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::ViewNotEnvironmentAllocatable { .. })
            ));
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let mut noncanonical = availability.plan().clone();
            let retained_row = noncanonical
                .classes
                .iter_mut()
                .find(|row| !row.unconstrained_views.is_empty())
                .unwrap();
            retained_row.unconstrained_views.push(sole_view);
            assert_eq!(
                validate_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    noncanonical,
                ),
                Err(TerminalAllocatorAvailabilityError::NonCanonicalPlan)
            );
            let encoded = availability.plan().encode();
            assert_eq!(
                omega_regalloc::TerminalAllocatorAvailabilityPlan::decode(&encoded).unwrap(),
                *availability.plan()
            );
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            if let Some(fixed_return) = fixed_return {
                assert_ne!(fixed_return, sole_view);
                assert!(
                    legality
                        .legality()
                        .plan()
                        .functions
                        .iter()
                        .flat_map(|function| &function.virtual_registers)
                        .flat_map(|register| &register.points)
                        .any(|point| point.candidates == vec![fixed_return])
                );
            }
            assert_eq!(
                legality.custody().allocator_availability(),
                legality.allocator_availability().receipt().identity()
            );
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            let choice = choices.plan().functions[0].choice.as_ref().unwrap();
            assert_eq!(choice.incoming, TerminalVirtualRegisterId(2));
            assert_eq!(choice.selected_victim, choice.incoming);
            assert_eq!(choice.incoming_common_candidates, vec![sole_view]);

            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            let row = recovery.plan().functions[0]
                .classification
                .as_ref()
                .unwrap();
            assert_eq!(row.victim, TerminalVirtualRegisterId(2));
            assert_eq!(row.role, TerminalRecoveryVictimRole::Incoming);
            assert!(matches!(
                row.classification,
                TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
                    value: IntegerValue::Unsigned(8),
                    ..
                }
            ));
        }
    }

    #[test]
    fn two_explicit_u12_exact_add_folds_close_one_view_pressure_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rax"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                budget(),
            )
            .unwrap();
            let fold_one = fold_terminal_selected_incoming_literal(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                &recovery,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(fold_one.receipt().applied_count(), 1);
            assert_eq!(
                TerminalLiteralFoldPlan::decode(&fold_one.plan().encode()).unwrap(),
                *fold_one.plan()
            );
            let mut corrupted_recipe = fold_one.plan().clone();
            corrupted_recipe.functions[0]
                .action
                .as_mut()
                .unwrap()
                .immediate += 1;
            assert!(matches!(
                validate_terminal_literal_fold(
                    selected.selected(),
                    ranges.ranges(),
                    legality.legality(),
                    &choices,
                    &recovery,
                    legality.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted_recipe,
                ),
                Err(omega_regalloc::TerminalLiteralFoldError::DecisionMismatch { .. })
            ));
            let foreign_target = match target.architecture {
                omega_target::Architecture::X86_64 => NativeTarget::linux_arm64(),
                omega_target::Architecture::Aarch64 => NativeTarget::linux_x64(),
            };
            let foreign_environment = baseline_target_register_environment(foreign_target).unwrap();
            assert_eq!(
                validate_terminal_literal_fold(
                    selected.selected(),
                    ranges.ranges(),
                    legality.legality(),
                    &choices,
                    &recovery,
                    legality.allocator_availability(),
                    environment.identity(),
                    foreign_environment.physical(),
                    foreign_environment.constraints(),
                    foreign_environment.reservations(),
                    environment.allocation_constraint_keys(),
                    fold_one.plan().clone(),
                ),
                Err(omega_regalloc::TerminalLiteralFoldError::RootMismatch)
            );
            let folded_add = &fold_one.transformed().functions[0].blocks[1].instructions[1];
            assert!(matches!(
                folded_add.kind,
                TerminalSelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(8),
                    ..
                }
            ));
            assert_eq!(folded_add.provenance.operations.len(), 2);
            assert_eq!(folded_add.provenance.obligations.len(), 1);

            let liveness_one = analyze_terminal_liveness(&fold_one).unwrap();
            let ranges_one = analyze_terminal_live_ranges(&fold_one, &liveness_one).unwrap();
            let legality_one = analyze_terminal_allocation_legality(
                &ranges_one,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            let choices_one = choose_terminal_spill_victims(
                &legality_one,
                &ranges_one,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            assert_eq!(
                choices_one.plan().functions[0]
                    .choice
                    .as_ref()
                    .unwrap()
                    .incoming,
                TerminalVirtualRegisterId(4)
            );
            let recovery_one = classify_terminal_pressure_recovery(
                &fold_one,
                &ranges_one,
                &legality_one,
                &choices_one,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                budget(),
            )
            .unwrap();
            let fold_two = fold_terminal_selected_incoming_literal(
                &fold_one,
                &ranges_one,
                &legality_one,
                &choices_one,
                &recovery_one,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(fold_two.receipt().applied_count(), 1);

            let liveness_two = analyze_terminal_liveness(&fold_two).unwrap();
            let ranges_two = analyze_terminal_live_ranges(&fold_two, &liveness_two).unwrap();
            let legality_two = analyze_terminal_allocation_legality(
                &ranges_two,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            let choices_two = choose_terminal_spill_victims(
                &legality_two,
                &ranges_two,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            assert!(
                choices_two
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.choice.is_none())
            );
            let homes = omega_regalloc::assign_terminal_register_homes(
                &legality_two,
                &ranges_two,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            assert_eq!(
                homes.plan().functions[0].assignments.len(),
                fold_two.transformed().functions[0].virtual_registers.len()
            );

            let staged_folds = stage_first_optimized_literal_fold(
                legality,
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(staged_folds.steps().len(), 1);
            let staged_environment = staged_folds
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            assert!(matches!(
                omega_regalloc::assign_terminal_register_homes(
                    staged_folds.final_step().legality(),
                    staged_folds.final_step().ranges(),
                    staged_environment.identity(),
                    staged_environment.physical(),
                    staged_environment.constraints(),
                    staged_environment.reservations(),
                    staged_environment.allocation_constraint_keys(),
                ),
                Err(TerminalRegisterHomeError::NoCompatibleHome { .. })
            ));
            let staged_folds = stage_next_optimized_literal_fold(
                staged_folds,
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(staged_folds.steps().len(), 2);
            assert_eq!(staged_folds.custody().transformations().len(), 2);
            let [first_iteration, second_iteration] = staged_folds.custody().iterations() else {
                panic!("two explicit fold calls must retain two iteration receipts");
            };
            assert_eq!(
                first_iteration.transformed_selected(),
                second_iteration.source_selected()
            );
            assert_eq!(
                first_iteration.fresh_ranges(),
                second_iteration.source_ranges()
            );
            assert_eq!(
                first_iteration.fresh_legality(),
                second_iteration.source_legality()
            );
            assert_eq!(
                second_iteration.fold_policy(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1
            );
            assert_eq!(
                validate_optimized_literal_fold_custody(&staged_folds).unwrap(),
                *staged_folds.custody()
            );
            assert_eq!(
                staged_folds.custody().final_selected(),
                staged_folds
                    .final_step()
                    .fold()
                    .receipt()
                    .transformed_selected()
            );
            let machine_effects =
                stage_optimized_machine_effects_after_literal_folds(&staged_folds).unwrap();
            assert_eq!(
                machine_effects.effects().receipt().selected(),
                staged_folds.custody().final_selected()
            );
            assert_eq!(
                machine_effects.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::LiteralFolds(
                    staged_folds.custody().clone()
                )
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody_after_literal_folds(
                    &staged_folds,
                    machine_effects.effects(),
                )
                .unwrap(),
                machine_effects.custody()
            );
            let expected_transformations = staged_folds
                .custody()
                .transformations()
                .iter()
                .copied()
                .map(PostAllocationSelectedTransformation::LiteralFold)
                .collect::<Vec<_>>();
            let staged_homes =
                stage_optimized_register_homes_after_literal_folds(staged_folds).unwrap();
            let post =
                stage_optimized_post_allocation_machine_plan_after_literal_folds(&staged_homes)
                    .unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                staged_homes.fold_stage().custody().final_selected()
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_literal_fold_custody(
                    &staged_homes,
                    &post,
                )
                .unwrap(),
                post.custody()
            );
            assert_eq!(
                staged_homes
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                expected_transformations
            );
            assert_eq!(
                staged_homes.post_allocation_manifest().record().selected,
                staged_homes.fold_stage().custody().final_selected()
            );
            assert_eq!(
                validate_optimized_register_home_after_literal_fold_custody(&staged_homes).unwrap(),
                *staged_homes.custody()
            );
        }
    }

    #[test]
    fn named_selected_lowering_suite_reaches_a_verified_fixed_point_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rax"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let selections = OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap();
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            let run = run_selected_lowering_optimizations(legality).unwrap();

            assert_eq!(run.selections(), &selections);
            assert_eq!(run.custody().selections(), selections.identity());
            assert_eq!(
                run.selected_lowering_selections().as_slice(),
                &[Optimization::SelectedIncomingU12ExactAddImmediate]
            );
            assert_eq!(
                run.custody().selected_lowering_selections(),
                run.selected_lowering_selections().identity()
            );
            assert_eq!(run.steps().len(), 2);
            assert_eq!(run.custody().action_count(), 2);
            assert_eq!(
                run.custody()
                    .initial_virtual_register_count()
                    .checked_sub(run.custody().action_count()),
                Some(run.custody().final_virtual_register_count())
            );
            assert_eq!(run.custody().iterations().len(), 2);
            assert_eq!(run.terminal_attempt().fold().receipt().applied_count(), 0);
            assert_eq!(
                run.terminal_attempt().fold().receipt().source_selected(),
                run.terminal_attempt()
                    .fold()
                    .receipt()
                    .transformed_selected()
            );
            assert!(run.custody().usage().within(run.custody().budget()));
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );
            let completion = run.custody().identity();
            let final_selected = run.custody().final_selected();
            let expected_transformations = run
                .custody()
                .iterations()
                .iter()
                .map(|iteration| {
                    PostAllocationSelectedTransformation::LiteralFold(iteration.fold())
                })
                .collect::<Vec<_>>();
            let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
            assert_eq!(
                homes
                    .post_allocation_manifest()
                    .record()
                    .selected_lowering_completion,
                Some(completion)
            );
            assert_eq!(
                homes
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                expected_transformations
            );
            assert_eq!(
                validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
                *homes.custody()
            );
            let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
            let post = realization.machine();
            assert_eq!(post.machine().receipt().selected(), final_selected);
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                    realization.homes(),
                    post,
                )
                .unwrap(),
                *post.custody()
            );
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization)
                    .unwrap(),
                *realization.custody()
            );
            let manifest = realization.manifest().record();
            assert_eq!(manifest.selections, selections.identity());
            assert_eq!(manifest.selected_lowering_completion, completion);
            assert_eq!(manifest.selected, final_selected);
            assert_eq!(manifest.pre_layout, realization.encoding().identity());
            assert_eq!(manifest.resolved_layout, realization.layout().identity());
            assert_eq!(
                manifest.whole_function_exit_contract,
                realization.exit_contract().identity()
            );
            assert_eq!(
                realization.exit_contract().contract().functions[0]
                    .returns
                    .len(),
                2
            );
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.blocks, 3);
            assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
            assert_eq!(
                manifest.statistics.bytes,
                realization
                    .layout()
                    .functions()
                    .iter()
                    .map(|function| function.byte_count)
                    .sum()
            );
        }
    }

    #[test]
    fn named_selected_lowering_suite_retains_verified_no_change_completion() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let selections = OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap();
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                        target,
                        selections.clone(),
                        selected_lowering_budget(),
                    ))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let source_selected = legality
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .receipt()
                .identity();
            let source_ranges = legality.live_range_stage().ranges().receipt().identity();
            let source_legality = legality.legality().receipt().identity();
            let run = run_selected_lowering_optimizations(legality).unwrap();

            assert!(run.steps().is_empty());
            assert_eq!(run.custody().action_count(), 0);
            assert_eq!(run.custody().final_selected(), source_selected);
            assert_eq!(run.custody().final_ranges(), source_ranges);
            assert_eq!(run.custody().final_legality(), source_legality);
            assert_eq!(run.terminal_attempt().fold().receipt().applied_count(), 0);
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );
            let completion = run.custody().identity();
            let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
            let manifest = homes.post_allocation_manifest().record();
            assert_eq!(manifest.selected_lowering_completion, Some(completion));
            assert!(manifest.selected_transformations.is_empty());
            assert_eq!(manifest.selected, source_selected);
            let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
            let post = realization.machine();
            assert_eq!(post.machine().receipt().selected(), source_selected);
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                    realization.homes(),
                    post,
                )
                .unwrap(),
                *post.custody()
            );
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization)
                    .unwrap(),
                *realization.custody()
            );
            let manifest = realization.manifest().record();
            assert_eq!(manifest.selected_lowering_completion, completion);
            assert_eq!(manifest.selected, source_selected);
            assert_eq!(
                manifest.whole_function_exit_contract,
                realization.exit_contract().identity()
            );
            assert_eq!(realization.exit_contract().contract().functions.len(), 1);
            assert_eq!(
                realization.exit_contract().contract().functions[0]
                    .returns
                    .len(),
                2
            );
            assert!(
                realization.exit_contract().contract().functions[0]
                    .modified_callee_saved_units
                    .is_empty()
            );
            let exit = realization.exit_contract().contract();
            assert_eq!(
                exit.hardening,
                TerminalWholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1
            );
            match (target.architecture, target.object_format) {
                (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Elf) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1
                    );
                    assert_eq!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                    );
                    assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                        returned.mechanism,
                        TerminalWholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                            read_bytes: 8,
                            pop_bytes: 8,
                            ..
                        }
                    )));
                }
                (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Coff) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
                    );
                    assert_eq!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                    );
                }
                (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::Elf) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1
                    );
                    assert!(matches!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                    ));
                    assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                        returned.mechanism,
                        TerminalWholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 { .. }
                    )));
                }
                (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::MachO) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1
                    );
                    assert!(matches!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                    ));
                }
                _ => unreachable!(),
            }
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.blocks, 3);
            assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
            assert_eq!(
                manifest.publication,
                FunctionRelativeOptimizationUnavailableData::Unavailable
            );
            let encoded = manifest.encode();
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&encoded),
                Ok(manifest.clone())
            );
            assert!(manifest.render_text().contains("publication: unavailable"));
            let mut identity_tamper = encoded.clone();
            identity_tamper[12] ^= 1;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&identity_tamper),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch)
            );
            let mut wrong_magic = encoded.clone();
            wrong_magic[0] ^= 1;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&wrong_magic),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic)
            );
            let mut wrong_version = encoded.clone();
            wrong_version[8..12].copy_from_slice(&3_u32.to_le_bytes());
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&wrong_version),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                        3
                    )
                )
            );
            let mut legacy_version = encoded.clone();
            legacy_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&legacy_version),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                        1
                    )
                )
            );
            let mut trailing = encoded.clone();
            trailing.push(0);
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&trailing),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes)
            );
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &encoded[..encoded.len() - 1]
                ),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
            );
            let content_offset = 8 + 4 + 32;
            let mut unknown_stage = encoded.clone();
            unknown_stage[content_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_stage),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(9))
            );
            let target_offset = content_offset + 1 + 11 * 32;
            let mut unknown_architecture = encoded.clone();
            unknown_architecture[target_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_architecture),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(
                        9
                    )
                )
            );
            let mut unknown_object_format = encoded.clone();
            unknown_object_format[target_offset + 1] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_object_format),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(
                        9
                    )
                )
            );
            let layout_policy_offset = target_offset + 2 + 8 + 8;
            let mut unknown_layout_policy = encoded.clone();
            unknown_layout_policy[layout_policy_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_layout_policy),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(
                        9
                    )
                )
            );
            let mut unknown_scope = encoded.clone();
            unknown_scope[layout_policy_offset + 1] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_scope),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(9))
            );
            let mut unknown_unavailable = encoded.clone();
            *unknown_unavailable.last_mut().unwrap() = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_unavailable),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(9))
            );

            let mut corrupted = realization;
            macro_rules! assert_manifest_field_is_bound {
                ($field:ident, $replacement:expr) => {{
                    let original = corrupted.manifest().record().$field;
                    corrupted.manifest_mut().record_mut().$field = $replacement;
                    assert_eq!(
                        validate_selected_lowering_function_relative_realization_custody(
                            &corrupted
                        ),
                        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
                    );
                    corrupted.manifest_mut().record_mut().$field = original;
                }};
            }
            assert_manifest_field_is_bound!(
                identity,
                omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
                    [0x50; 32]
                )
            );
            assert_manifest_field_is_bound!(
                selections,
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x51; 32])
            );
            assert_manifest_field_is_bound!(
                selected_lowering_selections,
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x52; 32])
            );
            assert_manifest_field_is_bound!(
                selected_lowering_completion,
                omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                    [0x53; 32]
                )
            );
            assert_manifest_field_is_bound!(
                pre_physical_manifest,
                omega_optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                    [0x54; 32]
                )
            );
            assert_manifest_field_is_bound!(
                post_allocation_manifest,
                omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                    [0x55; 32]
                )
            );
            assert_manifest_field_is_bound!(
                selected,
                omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_bytes(
                    [0x56; 32]
                )
            );
            assert_manifest_field_is_bound!(
                pre_allocation_machine_effects,
                omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity::from_bytes(
                    [0x57; 32]
                )
            );
            assert_manifest_field_is_bound!(
                post_allocation_machine,
                omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes(
                    [0x58; 32]
                )
            );
            assert_manifest_field_is_bound!(
                pre_layout,
                TerminalSelectedFormEncodingIdentity::from_bytes([0x59; 32])
            );
            assert_manifest_field_is_bound!(
                resolved_layout,
                TerminalResolvedSelectedFormLayoutIdentity::from_bytes([0x5a; 32])
            );
            assert_manifest_field_is_bound!(
                whole_function_exit_contract,
                TerminalWholeFunctionExitContractIdentity::from_bytes([0x5b; 32])
            );
            assert_manifest_field_is_bound!(
                target,
                if target == NativeTarget::linux_x64() {
                    NativeTarget::linux_arm64()
                } else {
                    NativeTarget::linux_x64()
                }
            );
            let original_bytes = corrupted.manifest().record().statistics.bytes;
            corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes + 1;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
            );
            corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes;
            let original_result_view = corrupted.exit_contract().contract().result_view;
            corrupted.exit_contract_mut().contract_mut().result_view = RegisterViewId(u16::MAX);
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().result_view = original_result_view;
            let original_exit_identity = corrupted.exit_contract().identity();
            corrupted.exit_contract_mut().contract_mut().identity =
                TerminalWholeFunctionExitContractIdentity::from_bytes([0x61; 32]);
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().identity = original_exit_identity;
            let original_offset =
                corrupted.exit_contract().contract().functions[0].returns[0].offset;
            corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
                original_offset + 1;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
                original_offset;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted)
                    .unwrap(),
                *corrupted.custody()
            );
        }
    }

    #[test]
    fn frameless_exit_contract_rejects_unpreserved_x86_callee_saved_write() {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections,
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let run = run_selected_lowering_optimizations(legality).unwrap();
        assert!(run.steps().is_empty());
        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        let rbx_units = homes
            .selected_lowering_run()
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .physical()
            .model()
            .view_named("rbx")
            .unwrap()
            .units
            .clone();
        let error = stage_selected_lowering_function_relative_realization(homes).unwrap_err();
        let FunctionRelativeOptimizationRealizationError::ExitContract(
            TerminalWholeFunctionExitContractError::CalleeSavedWrite { instruction, unit },
        ) = error
        else {
            panic!("unpreserved RBX write must fail at the whole-function exit contract")
        };
        assert_eq!(
            instruction,
            omega_terminal_selected_instructions::TerminalSelectedInstructionId(3)
        );
        assert!(rbx_units.contains(&unit));
    }

    #[test]
    fn compiler_facing_physical_pipeline_routes_psi_only_and_selected_lowering_suites() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = conditional_exact_binary_artifact(false);
            let psi_only_selections =
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(
                    psi_only_selections.clone(),
                    selected_lowering_budget(),
                )
                .unwrap(),
            )
            .unwrap();
            let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            assert!(matches!(
                staged,
                StagedOptimizedVerifiedPhysicalPipeline::PsiOnly { .. }
            ));
            assert_eq!(staged.selections(), psi_only_selections.identity());
            assert_eq!(staged.selected_lowering_completion(), None);
            assert!(staged.function_relative_realization().is_none());

            for selections in [
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SelectedIncomingU12ExactAddImmediate,
                ])
                .unwrap(),
                OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                    .unwrap(),
            ] {
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    ExplicitOptimizationRequest::new(
                        selections.clone(),
                        selected_lowering_budget(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    target,
                    &[],
                )
                .unwrap();
                let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } =
                    &staged
                else {
                    panic!("selected-lowering phase must run when its exact family is selected")
                };
                let homes = realization.homes();
                let machine = realization.machine();
                assert_eq!(staged.selections(), selections.identity());
                assert_eq!(
                    staged.selected_lowering_completion(),
                    Some(homes.selected_lowering_run().custody().identity())
                );
                assert_eq!(
                    staged.function_relative_realization().unwrap().custody(),
                    realization.custody()
                );
                assert!(homes.selected_lowering_run().steps().is_empty());
                assert_eq!(
                    machine.machine().receipt().post_allocation_manifest(),
                    homes.post_allocation_manifest().record().identity
                );
                assert_eq!(
                    realization.manifest().record().selections,
                    selections.identity()
                );
                assert_eq!(
                    realization.manifest().record().publication,
                    FunctionRelativeOptimizationUnavailableData::Unavailable
                );
            }
        }
    }

    #[test]
    fn selected_lowering_suite_enforces_one_aggregate_budget() {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let reference = run_selected_lowering_optimizations(source).unwrap();
        let attempt = reference.terminal_attempt();
        let component_usages = [
            attempt.choices().receipt().usage(),
            attempt.recovery().receipt().usage(),
            attempt.fold().receipt().usage(),
        ];
        let maximum = |field: fn(OptimizationWorkUsage) -> u64| {
            component_usages
                .into_iter()
                .map(field)
                .max()
                .unwrap()
                .max(1)
        };
        let component_only_budget = OptimizationWorkBudget::new(
            maximum(|usage| usage.rule_evaluations),
            maximum(|usage| usage.candidates),
            maximum(|usage| usage.validation_steps),
            maximum(|usage| usage.commits),
            maximum(|usage| usage.iterations),
        )
        .unwrap();
        assert!(
            component_usages
                .into_iter()
                .all(|usage| usage.within(component_only_budget))
        );

        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections,
                    component_only_budget,
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            run_selected_lowering_optimizations(source),
            Err(OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded { .. })
        ));
    }

    #[test]
    fn selected_lowering_runner_rejects_a_psi_only_source_suite() {
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            run_selected_lowering_optimizations(legality),
            Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization)
        ));
    }

    #[test]
    fn literal_fold_staging_rejects_an_explicit_no_action_request() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(matches!(
                stage_first_optimized_literal_fold(
                    legality,
                    TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                    budget(),
                ),
                Err(OptimizedLiteralFoldCustodyError::NoAppliedFold)
            ));
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

    fn validate_raw_post_allocation(
        source: &StagedOptimizedRegisterHomes,
        staged: &StagedOptimizedPostAllocationMachinePlan,
        raw: omega_machine_optimizer::TerminalPostAllocationMachinePlan,
    ) -> Result<
        omega_machine_optimizer::ValidatedTerminalPostAllocationMachinePlan,
        omega_machine_optimizer::TerminalPostAllocationMachineError,
    > {
        let selected = source
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let environment = selected.register_environment();
        omega_machine_optimizer::validate_terminal_post_allocation_machine_plan(
            selected.selected(),
            staged.effects().effects(),
            source.legality_stage().live_range_stage().ranges(),
            source.legality_stage().legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
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

    #[test]
    fn physical_stage_receipts_retain_the_pre_physical_manifest_identity() {
        let selected = staged_conditional(NativeTarget::linux_x64());
        let manifest = selected
            .optimized_target()
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity;
        assert_eq!(selected.custody().manifest(), manifest);

        let liveness = stage_optimized_liveness(selected).unwrap();
        assert_eq!(liveness.custody().manifest(), manifest);
        let ranges = stage_optimized_live_ranges(liveness).unwrap();
        assert_eq!(ranges.custody().manifest(), manifest);
        let legality = stage_optimized_allocation_legality(ranges).unwrap();
        assert_eq!(legality.custody().manifest(), manifest);
        let homes = stage_optimized_register_homes(legality).unwrap();
        assert_eq!(homes.custody().manifest(), manifest);
    }

    fn named_units(staged: &StagedOptimizedLiveness, names: &[&str]) -> Vec<RegisterUnitId> {
        names
            .iter()
            .flat_map(|name| {
                staged
                    .selected_stage()
                    .register_environment()
                    .physical()
                    .model()
                    .view_named(name)
                    .unwrap()
                    .units
                    .iter()
                    .copied()
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[test]
    fn selected_liveness_is_exact_on_both_architectures() {
        for (target, before_compare, after_compare, after_branch) in [
            (
                NativeTarget::linux_x64(),
                vec!["rip", "rsp"],
                vec!["rflags", "rip", "rsp"],
                vec!["rsp"],
            ),
            (
                NativeTarget::linux_arm64(),
                vec!["pc", "sp", "x30"],
                vec!["nzcv", "pc", "sp", "x30"],
                vec!["sp", "x30"],
            ),
        ] {
            let staged = stage_optimized_liveness(staged_conditional(target)).unwrap();
            let plan = staged.liveness().plan();
            let function = &plan.functions[0];
            assert_eq!(function.entry_definitions.len(), 1);
            assert_eq!(
                function.entry_definitions[0].virtual_register,
                TerminalVirtualRegisterId(0)
            );
            assert!(function.entry_definitions[0].fixed_view.is_some());
            assert_eq!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .map(|instruction| instruction.position.0)
                    .collect::<Vec<_>>(),
                (0..6).collect::<Vec<_>>()
            );

            let entry = &function.blocks[0];
            assert_eq!(entry.virtual_live_in, vec![TerminalVirtualRegisterId(0)]);
            assert!(entry.virtual_live_out.is_empty());
            assert_eq!(
                entry.instructions[0].virtual_uses,
                vec![TerminalVirtualRegisterId(0)]
            );
            assert!(entry.instructions[0].virtual_defs.is_empty());
            assert_eq!(
                entry.instructions[0].unit_live_in,
                named_units(&staged, &before_compare)
            );
            assert_eq!(
                entry.instructions[0].unit_live_out,
                named_units(&staged, &after_compare)
            );
            assert_eq!(
                entry.instructions[1].unit_live_in,
                entry.instructions[0].unit_live_out
            );
            assert_eq!(
                entry.instructions[1].unit_live_out,
                named_units(&staged, &after_branch)
            );
            assert_eq!(entry.successors.len(), 2);
            assert_eq!(entry.successors[0].polarity_ordinal, 0);
            assert_eq!(entry.successors[1].polarity_ordinal, 1);
            for successor in &entry.successors {
                let target_block = &function.blocks[successor.target.0 as usize];
                assert_eq!(successor.virtual_live, target_block.virtual_live_in);
                assert_eq!(successor.unit_live, target_block.unit_live_in);
            }

            for (block, register) in function.blocks[1..]
                .iter()
                .zip([TerminalVirtualRegisterId(1), TerminalVirtualRegisterId(2)])
            {
                assert!(block.virtual_live_in.is_empty());
                assert!(block.virtual_live_out.is_empty());
                assert_eq!(block.instructions[0].virtual_defs, vec![register]);
                assert_eq!(block.instructions[0].virtual_live_out, vec![register]);
                assert_eq!(block.instructions[1].virtual_uses, vec![register]);
                assert_eq!(block.instructions[1].virtual_live_in, vec![register]);
                assert!(block.instructions[1].virtual_live_out.is_empty());
            }
            assert_eq!(staged.custody().function_count(), 1);
            assert_eq!(staged.custody().block_count(), 3);
            assert_eq!(staged.custody().virtual_register_count(), 3);
            assert_eq!(staged.custody().instruction_count(), 6);
            assert_eq!(staged.custody().successor_count(), 2);
            assert_eq!(
                staged.custody().register_environment(),
                staged.selected_stage().register_environment().identity()
            );
            assert_eq!(
                staged.custody().liveness(),
                staged.liveness().receipt().identity()
            );
            assert_eq!(
                staged.custody().selected(),
                staged.selected_stage().selected().receipt().identity()
            );
        }
    }

    #[test]
    fn forwarded_parameter_conditional_retains_cross_edge_liveness() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_forwarded_conditional(target);
            let selected_plan = selected.selected().plan();
            assert_eq!(selected_plan.functions[0].virtual_registers.len(), 2);
            assert_eq!(
                selected_plan.functions[0]
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len() + 1)
                    .sum::<usize>(),
                4
            );
            assert!(
                selected_plan.functions[0].virtual_registers[0]
                    .entry_fixed_view
                    .is_some()
            );
            assert!(
                selected_plan.functions[0].virtual_registers[1]
                    .entry_fixed_view
                    .is_some()
            );

            let staged = stage_optimized_liveness(selected).unwrap();
            let function = &staged.liveness().plan().functions[0];
            assert_eq!(
                function.blocks[0].virtual_live_in,
                vec![TerminalVirtualRegisterId(0), TerminalVirtualRegisterId(1)]
            );
            assert_eq!(
                function.blocks[0].virtual_live_out,
                vec![TerminalVirtualRegisterId(1)]
            );
            for successor in &function.blocks[0].successors {
                assert_eq!(successor.virtual_live, vec![TerminalVirtualRegisterId(1)]);
            }
            for block in &function.blocks[1..] {
                assert_eq!(block.virtual_live_in, vec![TerminalVirtualRegisterId(1)]);
                assert!(block.virtual_live_out.is_empty());
                assert_eq!(
                    block.instructions[0].virtual_uses,
                    vec![TerminalVirtualRegisterId(1)]
                );
                assert!(block.instructions[0].virtual_live_out.is_empty());
                assert!(block.instructions[0].unit_live_out.is_empty());
            }
        }
    }

    #[test]
    fn forwarded_parameter_selection_rejects_fixed_input_and_path_corruption() {
        let staged = staged_forwarded_conditional(NativeTarget::linux_x64());
        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].virtual_registers[1].entry_fixed_view = None;
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
        instruction.operands[0].virtual_register = TerminalVirtualRegisterId(0);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));
    }

    #[test]
    fn live_ranges_are_block_local_and_interference_is_cfg_exact() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            let function = &staged.ranges().plan().functions[0];
            assert_eq!(
                function
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 6), (2, 6, 8)]
            );
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(
                function.virtual_registers[0].fragments,
                vec![TerminalLiveRangeFragment {
                    block: omega_terminal_selected_instructions::TerminalSelectedBlockId(0),
                    start: TerminalLiveRangePoint(0),
                    end: TerminalLiveRangePoint(1),
                }]
            );
            assert_eq!(
                function.virtual_registers[1]
                    .fragments
                    .iter()
                    .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 5), (2, 6, 7)]
            );
            assert_eq!(
                function.virtual_registers[1]
                    .edge_connectors
                    .iter()
                    .map(|edge| (edge.polarity_ordinal, edge.psi_edge, edge.target.0))
                    .collect::<Vec<_>>(),
                vec![
                    (0, EdgeId::new(4_011).unwrap(), 1),
                    (1, EdgeId::new(4_012).unwrap(), 2),
                ]
            );
            assert_eq!(
                function.interference,
                vec![TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(0),
                    higher: TerminalVirtualRegisterId(1),
                }]
            );
            assert_eq!(function.virtual_registers[0].fixed_constraints.len(), 1);
            assert!(matches!(
                function.virtual_registers[0].fixed_constraints[0].site,
                TerminalVirtualFixedConstraintSite::Entry
            ));
            assert_eq!(function.virtual_registers[1].fixed_constraints.len(), 3);
            assert!(matches!(
                function.virtual_registers[1].fixed_constraints[0].site,
                TerminalVirtualFixedConstraintSite::Entry
            ));
            assert!(
                function.virtual_registers[1].fixed_constraints[1..]
                    .iter()
                    .all(|constraint| matches!(
                        constraint.site,
                        TerminalVirtualFixedConstraintSite::Operand { .. }
                    ))
            );
            assert_eq!(staged.custody().interference_count(), 1);
            assert_eq!(
                staged.custody().register_environment(),
                staged
                    .liveness_stage()
                    .selected_stage()
                    .register_environment()
                    .identity()
            );
            assert_eq!(
                staged.custody().ranges(),
                staged.ranges().receipt().identity()
            );
            assert_eq!(
                staged.custody().liveness(),
                staged.liveness_stage().liveness().receipt().identity()
            );

            let repeated = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            assert_eq!(staged.ranges(), repeated.ranges());
            assert_eq!(staged.custody(), repeated.custody());
        }

        let constant = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
        )
        .unwrap();
        let function = &constant.ranges().plan().functions[0];
        assert_eq!(
            function
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
        );
        assert_eq!(
            function
                .virtual_registers
                .iter()
                .flat_map(|range| &range.fragments)
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 1), (1, 5, 7), (2, 9, 11)]
        );
        assert!(function.interference.is_empty());
        assert!(
            function
                .virtual_registers
                .iter()
                .all(|range| range.edge_connectors.is_empty())
        );
    }

    #[test]
    fn allocation_legality_is_phase_exact_and_exposes_fixed_view_transitions() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let function = &staged.legality().plan().functions[0];
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(function.virtual_registers[0].entry_transitions.len(), 0);
            assert_eq!(function.virtual_registers[1].entry_transitions.len(), 2);
            assert!(
                function.virtual_registers[1]
                    .entry_transitions
                    .iter()
                    .all(|transition| transition.from_view != transition.to_view)
            );

            let environment = staged
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let model = environment.physical().model();
            let range_function = &staged.live_range_stage().ranges().plan().functions[0];
            for register in &function.virtual_registers {
                for point in &register.points {
                    assert!(!point.candidates.is_empty());
                    for candidate in &point.candidates {
                        let view = &model.views[usize::from(candidate.0)];
                        assert_eq!(view.class, register.class);
                        assert!(view.allocatable);
                        assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                            environment
                                .reservations()
                                .reserved_units()
                                .binary_search(unit)
                                .is_err()
                        }));
                        assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                            range_function
                                .architectural_units
                                .iter()
                                .find(|row| row.unit == *unit)
                                .is_none_or(|row| {
                                    !row.fragments.iter().any(|fragment| {
                                        fragment.block == point.block
                                            && fragment.start <= point.point
                                            && point.point < fragment.end
                                    }) && !row.actions.iter().any(|action| {
                                        action.block == point.block && action.point == point.point
                                    })
                                })
                        }));
                    }
                }
            }
            assert_eq!(
                staged.custody().register_environment(),
                environment.identity()
            );
            assert_eq!(
                staged.custody().legality(),
                staged.legality().receipt().identity()
            );
            assert_eq!(staged.custody().entry_transition_count(), 2);

            if target == NativeTarget::linux_x64() {
                let mut overlays = environment.reservations().profile().active_overlays.clone();
                overlays.retain(|name| name != "omega.x86.metering");
                let reduced = validate_register_reservation_profile(
                    RegisterReservationProfile {
                        name: "test.no-metering".into(),
                        active_overlays: overlays,
                    },
                    target,
                    environment.physical(),
                )
                .unwrap();
                let reduced_identity = target_register_environment_identity(
                    target,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                );
                let reduced_availability = materialize_terminal_allocator_availability(
                    reduced_identity,
                    target,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
                )
                .unwrap();
                let reduced_legality = omega_regalloc::analyze_terminal_allocation_legality(
                    staged.live_range_stage().ranges(),
                    &reduced_availability,
                    reduced_identity,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                )
                .unwrap();
                let r15 = model.view_named("r15").unwrap().id;
                assert!(
                    function
                        .virtual_registers
                        .iter()
                        .flat_map(|register| &register.points)
                        .all(|point| !point.candidates.contains(&r15))
                );
                assert!(
                    reduced_legality.plan().functions[0]
                        .virtual_registers
                        .iter()
                        .flat_map(|register| &register.points)
                        .any(|point| point.candidates.contains(&r15))
                );
                assert_ne!(
                    reduced_legality.receipt().identity(),
                    staged.legality().receipt().identity()
                );
            }

            let repeated = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(staged.legality(), repeated.legality());
            assert_eq!(staged.custody(), repeated.custody());

            let mut corrupted = staged.legality().plan().clone();
            corrupted.functions[0].virtual_registers[0].points[0]
                .candidates
                .clear();
            assert_ne!(
                terminal_allocation_legality_identity(&corrupted),
                staged.legality().receipt().identity()
            );
            let ranges = staged.live_range_stage();
            assert!(matches!(
                validate_terminal_allocation_legality(
                    ranges.ranges(),
                    staged.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalAllocationLegalityError::VirtualRegisterMismatch { .. })
            ));
        }

        let constant = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(constant.custody().entry_transition_count(), 0);
    }

    #[test]
    fn transition_free_register_homes_are_deterministic_and_cfg_exact() {
        for (target, condition_view, result_view) in [
            (NativeTarget::linux_x64(), "rdi", "rax"),
            (NativeTarget::linux_arm64(), "x0", "x0"),
        ] {
            let staged = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged_conditional(target)).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let function = &staged.homes().plan().functions[0];
            assert_eq!(function.assignments.len(), 3);
            let environment = staged
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let encoded = staged.homes().plan().encode();
            let decoded = TerminalRegisterHomePlan::decode(&encoded).unwrap();
            assert_eq!(&decoded, staged.homes().plan());
            let legality = staged.legality_stage();
            let ranges = legality.live_range_stage();
            let replay = validate_terminal_register_homes(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                decoded,
            )
            .unwrap();
            assert_eq!(replay, *staged.homes());
            let manifest = staged.post_allocation_manifest().record();
            assert_eq!(manifest.identity, manifest.recomputed_identity());
            assert_eq!(
                PostAllocationOptimizationManifest::decode(&manifest.encode()),
                Ok(manifest.clone())
            );
            assert_eq!(manifest.pre_physical, staged.custody().manifest());
            assert_eq!(manifest.target, target);
            assert!(manifest.selected_transformations.is_empty());
            assert_eq!(manifest.homes, staged.homes().receipt().identity());
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.assignments, 3);
            assert_eq!(manifest.statistics.fixed_view_transitions, 0);
            assert_eq!(
                staged.custody().post_allocation_manifest(),
                manifest.identity
            );
            assert_eq!(
                validate_optimized_register_home_custody(
                    legality,
                    staged.homes(),
                    staged.post_allocation_manifest(),
                )
                .unwrap(),
                staged.custody()
            );
            assert!(manifest.render_text().contains("frame: unavailable"));
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    manifest,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                )
                .unwrap(),
                *staged.post_allocation_manifest()
            );
            let mut corrupted = manifest.clone();
            corrupted.statistics.assignments += 1;
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    &corrupted,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::IdentityMismatch)
            );
            corrupted.identity = corrupted.recomputed_identity();
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    &corrupted,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::ContentMismatch)
            );
            let model = environment.physical().model();
            assert_eq!(
                function.assignments[0].view,
                model.view_named(condition_view).unwrap().id
            );
            assert_eq!(
                function.assignments[1].view,
                model.view_named(result_view).unwrap().id
            );
            assert_eq!(function.assignments[1].view, function.assignments[2].view);
            assert!(
                staged
                    .legality_stage()
                    .live_range_stage()
                    .ranges()
                    .plan()
                    .functions[0]
                    .interference
                    .is_empty()
            );
            for assignment in &function.assignments {
                let view = &model.views[usize::from(assignment.view.0)];
                assert_eq!(view.class, assignment.class);
                assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                    environment
                        .reservations()
                        .reserved_units()
                        .binary_search(unit)
                        .is_err()
                }));
            }
            assert_eq!(staged.custody().assignment_count(), 3);
            assert_eq!(
                staged.custody().homes(),
                staged.homes().receipt().identity()
            );
            assert_eq!(
                staged.custody().register_environment(),
                environment.identity()
            );

            let repeated = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged_conditional(target)).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(staged.homes(), repeated.homes());
            assert_eq!(staged.custody(), repeated.custody());

            let mut corrupted = staged.homes().plan().clone();
            let original_view = corrupted.functions[0].assignments[0].view;
            corrupted.functions[0].assignments[0].view = model
                .views
                .iter()
                .find(|view| {
                    view.class == corrupted.functions[0].assignments[0].class
                        && view.id != original_view
                })
                .expect("fixture register class has a distinct corruption view")
                .id;
            assert_ne!(
                terminal_register_home_identity(&corrupted),
                staged.homes().receipt().identity()
            );
            let legality = staged.legality_stage();
            let ranges = legality.live_range_stage();
            assert!(matches!(
                validate_terminal_register_homes(
                    legality.legality(),
                    ranges.ranges(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalRegisterHomeError::VirtualRegisterMismatch { .. })
                    | Err(TerminalRegisterHomeError::UnknownOrIncompatibleView { .. })
            ));
        }

        let forwarded = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_register_homes(forwarded),
            Err(OptimizedRegisterHomeCustodyError::Assignment(
                TerminalRegisterHomeError::UnresolvedEntryTransitions { count: 2, .. }
            ))
        ));
    }

    #[test]
    fn fixed_view_copies_are_explicit_reanalyzed_and_deterministic() {
        for (target, entry_name, result_name) in [
            (NativeTarget::linux_x64(), "rsi", "rax"),
            (NativeTarget::linux_arm64(), "x1", "x0"),
        ] {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let source_selected = source
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .plan()
                .clone();
            let source_manifest = source.custody().manifest();
            let materialized = stage_optimized_fixed_view_copies(
                source,
                TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                budget(),
            )
            .unwrap();
            let machine_effects =
                stage_optimized_machine_effects_after_fixed_view_copies(&materialized).unwrap();
            assert_eq!(
                machine_effects.effects().receipt().selected(),
                materialized.custody().transformed_selected()
            );
            assert_eq!(
                machine_effects.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(
                    materialized.custody()
                )
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody_after_fixed_view_copies(
                    &materialized,
                    machine_effects.effects(),
                )
                .unwrap(),
                machine_effects.custody()
            );
            let copy_plan = materialized.copies().plan();
            assert_eq!(copy_plan.copies.len(), 2);
            assert_eq!(materialized.custody().copy_count(), 2);
            assert_eq!(materialized.custody().manifest(), source_manifest);
            assert_eq!(
                copy_plan.usage,
                omega_optimization_core::OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 2,
                    validation_steps: 2,
                    commits: 2,
                    iterations: 1,
                }
            );
            assert_ne!(
                materialized.custody().source_selected(),
                materialized.custody().transformed_selected()
            );
            assert_eq!(
                terminal_fixed_view_copy_identity(copy_plan),
                materialized.custody().transformation()
            );
            let transformed = &copy_plan.transformed;
            assert_eq!(transformed.functions[0].virtual_registers.len(), 4);
            let environment = materialized
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let entry_view = environment
                .physical()
                .model()
                .view_named(entry_name)
                .unwrap()
                .id;
            let result_view = environment
                .physical()
                .model()
                .view_named(result_name)
                .unwrap()
                .id;
            for (index, copy) in copy_plan.copies.iter().enumerate() {
                assert_eq!(copy.source_virtual_register, TerminalVirtualRegisterId(1));
                assert_eq!(copy.result_virtual_register.0, 2 + index as u32);
                assert_eq!(copy.copy_instruction.0, 4 + index as u32);
                assert_eq!(copy.from_view, entry_view);
                assert_eq!(copy.to_view, result_view);
                assert_eq!(copy.copy_constraint, environment.selected_keys().copy_i64);
                let block = &transformed.functions[0].blocks[index + 1];
                let instruction = block.instructions.last().unwrap();
                assert_eq!(instruction.id, copy.copy_instruction);
                assert_eq!(instruction.kind, TerminalSelectedInstructionKind::CopyI64);
                assert_eq!(
                    instruction.operands[0].virtual_register,
                    copy.source_virtual_register
                );
                assert_eq!(
                    instruction.operands[1].virtual_register,
                    copy.result_virtual_register
                );
                assert!(instruction.provenance.operations.is_empty());
                assert_eq!(instruction.provenance.values, vec![copy.source_value]);
                assert!(instruction.provenance.edges.is_empty());
                assert!(instruction.provenance.obligations.is_empty());
                assert!(instruction.provenance.fuel.is_empty());
                let TerminalSelectedTerminator::Return {
                    instruction: source_return,
                    ..
                } = &source_selected.functions[0].blocks[index + 1].terminator
                else {
                    unreachable!()
                };
                let TerminalSelectedTerminator::Return {
                    instruction: transformed_return,
                    ..
                } = &block.terminator
                else {
                    unreachable!()
                };
                assert_eq!(source_return.id, transformed_return.id);
                assert_eq!(source_return.provenance, transformed_return.provenance);
                assert_eq!(
                    transformed_return.operands[0].virtual_register,
                    copy.result_virtual_register
                );
            }

            let mut corrupted = materialized.copies().plan().clone();
            corrupted.copies[0].from_view = result_view;
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::CopyMismatch { index: 0 })
            ));
            let mut corrupted = materialized.copies().plan().clone();
            corrupted.transformed.functions[0].blocks[1].instructions[0]
                .provenance
                .values
                .clear();
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::TransformedPlanMismatch)
            ));
            let mut corrupted = materialized.copies().plan().clone();
            corrupted.usage.commits += 1;
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::ReceiptMismatch)
            ));
            assert!(matches!(
                validate_terminal_live_ranges(
                    materialized.copies(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .liveness(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges()
                        .plan()
                        .clone(),
                ),
                Err(TerminalLiveRangeError::LivenessRevalidation(
                    TerminalLivenessError::RootMismatch
                ))
            ));

            let reanalyzed = stage_optimized_selected_reanalysis(materialized).unwrap();
            assert_eq!(reanalyzed.custody().entry_transition_count(), 0);
            assert_eq!(reanalyzed.legality().receipt().entry_transition_count(), 0);
            let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalyzed).unwrap();
            let post = stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes)
                .unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                homes.reanalysis_stage().ranges().receipt().selected()
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                    &homes, &post,
                )
                .unwrap(),
                post.custody()
            );
            let assignments = &homes.homes().plan().functions[0].assignments;
            assert_eq!(assignments.len(), 4);
            assert_eq!(assignments[1].view, entry_view);
            assert_ne!(assignments[0].view, assignments[1].view);
            assert_eq!(assignments[2].view, result_view);
            assert_eq!(assignments[3].view, result_view);
            assert_eq!(
                homes.reanalysis_stage().ranges().plan().functions[0].interference,
                vec![TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(0),
                    higher: TerminalVirtualRegisterId(1),
                }]
            );
            assert_eq!(homes.custody().assignment_count(), 4);
            let manifest = homes.post_allocation_manifest().record();
            assert_eq!(manifest.identity, manifest.recomputed_identity());
            assert_eq!(
                PostAllocationOptimizationManifest::decode(&manifest.encode()),
                Ok(manifest.clone())
            );
            assert_eq!(
                manifest.selected_transformations,
                vec![PostAllocationSelectedTransformation::FixedViewCopy(
                    homes.custody().source().source().transformation()
                )]
            );
            assert_eq!(
                manifest.selected,
                homes.reanalysis_stage().ranges().plan().selected
            );
            assert_eq!(manifest.statistics.assignments, 4);
            assert_eq!(manifest.statistics.virtual_interferences, 1);
            let transformation = PostAllocationSelectedTransformation::FixedViewCopy(
                homes.custody().source().source().transformation(),
            );
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    manifest,
                    homes.custody().source().source().manifest(),
                    &[transformation, transformation],
                    homes.reanalysis_stage().ranges(),
                    homes.reanalysis_stage().legality(),
                    homes.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger)
            );
            assert_eq!(
                homes.custody().post_allocation_manifest(),
                manifest.identity
            );
            assert_eq!(
                validate_optimized_register_home_after_fixed_view_copy_custody(
                    homes.reanalysis_stage(),
                    homes.homes(),
                    homes.post_allocation_manifest(),
                )
                .unwrap(),
                homes.custody()
            );

            let repeated = stage_optimized_register_homes_after_fixed_view_copies(
                stage_optimized_selected_reanalysis(
                    stage_optimized_fixed_view_copies(
                        stage_optimized_allocation_legality(
                            stage_optimized_live_ranges(
                                stage_optimized_liveness(staged_forwarded_conditional(target))
                                    .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                        TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                        budget(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(homes.homes(), repeated.homes());
            assert_eq!(homes.custody(), repeated.custody());
        }

        let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_fixed_view_copies(
                source,
                TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                constrained,
            ),
            Err(OptimizedFixedViewCopyCustodyError::Materialization(
                TerminalFixedViewCopyError::BudgetExceeded { .. }
            ))
        ));

        let constant = stage_optimized_fixed_view_copies(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64()))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget(),
        )
        .unwrap();
        assert!(constant.copies().plan().copies.is_empty());
        assert_eq!(
            constant.copies().plan().source_selected,
            constant.copies().receipt().transformed_selected()
        );
    }

    #[test]
    fn architectural_actions_do_not_inflate_semantic_unit_fragments() {
        for (target, instruction_pointer) in [
            (NativeTarget::linux_x64(), "rip"),
            (NativeTarget::linux_arm64(), "pc"),
        ] {
            let staged = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            let unit = named_units(staged.liveness_stage(), &[instruction_pointer])[0];
            let range = staged.ranges().plan().functions[0]
                .architectural_units
                .iter()
                .find(|range| range.unit == unit)
                .unwrap();
            assert_eq!(
                range
                    .fragments
                    .iter()
                    .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 3)]
            );
            assert!(range.actions.iter().any(|action| {
                action.point == TerminalLiveRangePoint(3)
                    && action.kind == TerminalArchitecturalUnitActionKind::Def
            }));
        }
    }

    #[test]
    fn independent_live_range_validation_rejects_corruption_and_detachment() {
        let staged =
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap();
        let valid =
            analyze_terminal_live_ranges(staged.selected_stage().selected(), staged.liveness())
                .unwrap();
        let identity = terminal_live_range_identity(valid.plan());

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].fragments[0]
            .end
            .0 -= 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted.clone(),
            ),
            Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));
        assert_ne!(terminal_live_range_identity(&corrupted), identity);

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].edge_connectors[0].polarity_ordinal = 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::NonCanonicalRows { .. })
                | Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].interference.clear();
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::InterferenceMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].fixed_constraints[0]
            .view
            .0 += 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].architectural_units[0].actions[0]
            .point
            .0 += 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::ArchitecturalUnitMismatch { .. })
        ));

        let arm =
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_arm64()))
                .unwrap();
        let arm_ranges =
            analyze_terminal_live_ranges(arm.selected_stage().selected(), arm.liveness()).unwrap();
        assert!(matches!(
            validate_optimized_live_range_custody(&staged, &arm_ranges),
            Err(OptimizedLiveRangeCustodyError::Revalidation(
                TerminalLiveRangeError::RootMismatch
            ))
        ));
    }

    #[test]
    fn selected_liveness_is_deterministic_and_identity_binds_every_domain() {
        let first =
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
        let second =
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
        assert_eq!(first.liveness(), second.liveness());
        assert_eq!(first.custody(), second.custody());

        let original = first.liveness().plan();
        let identity = terminal_liveness_identity(original);
        let mut mutations = Vec::new();
        let mut changed = original.clone();
        changed.selected =
            omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(
                b"changed-selected",
            );
        mutations.push(changed);
        let mut changed = original.clone();
        changed.target = NativeTarget::windows_x64();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.fuel_schedule = psi_core::FuelScheduleIdentity::new(
            original.fuel_schedule.marker().checked_add(1).unwrap(),
        )
        .unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.optimization_unit =
            omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"changed");
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].machine = MachineId::new(8_101).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].fixed_view = None;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].virtual_register = TerminalVirtualRegisterId(8);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].position.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].instruction.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].operand += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].virtual_register.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].access = RegisterOperandAccess::Def;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].fixed_view =
            changed.functions[0].entry_definitions[0].fixed_view;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].tied_to = Some(0);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].early_clobber = true;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].block.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].source_block = BlockId::new(8_103).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .virtual_live_in
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .unit_live_in
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .virtual_live_out
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .unit_live_out
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0].position.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0].instruction.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .virtual_uses
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_live_in
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_live_out
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[1]
            .unit_uses
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_clobbers
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_live_in
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_live_out
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].polarity_ordinal = 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].psi_edge = EdgeId::new(8_102).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].terminator.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].target.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0]
            .virtual_live
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0]
            .unit_live
            .clear();
        mutations.push(changed);
        for mutation in mutations {
            assert_ne!(terminal_liveness_identity(&mutation), identity);
        }
    }

    #[test]
    fn independent_liveness_validator_rejects_raw_transfer_and_path_corruption() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_conditional(target);
            let valid = analyze_terminal_liveness(selected.selected()).unwrap();

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].virtual_live_in.clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::BlockMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].instructions[0]
                .unit_live_out
                .clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::TransferMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[1].instructions[1]
                .virtual_live_in
                .clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::TransferMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].successors.swap(0, 1);
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::SuccessorMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[2].instructions[0].position.0 = 99;
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::NonDensePositions { .. })
            ));
        }
    }

    #[test]
    fn liveness_custody_rejects_a_detached_same_shape_target() {
        let x86 = staged_conditional(NativeTarget::linux_x64());
        let arm = staged_conditional(NativeTarget::linux_arm64());
        let arm_liveness = analyze_terminal_liveness(arm.selected()).unwrap();
        assert!(matches!(
            validate_optimized_liveness_custody(&x86, &arm_liveness),
            Err(OptimizedLivenessCustodyError::Revalidation(
                TerminalLivenessError::RootMismatch
            ))
        ));
    }
}
