#![forbid(unsafe_code)]

//! Custody-preserving bridge from validated target-neutral optimization to
//! clean abstract-operation lowering.
//!
//! This crate does not perform target legalization or physical allocation yet.
//! It reifies the final optimizer unit into executable abstract-plan shape and
//! keeps that shape inseparable from its verifier context, candidate replay,
//! transformation ledger, and independent projection-validation receipt.

use omega_optimization_core::{
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationRuleSetIdentity,
    OptimizationSelections, OptimizationWorkUsage,
};
use omega_optimization_policy::BaselineDecisionLog;
use omega_optimization_unit::{
    PsiOptimizationUnit, PsiTransformationLedger, PsiTransformationRecord, ValueDefinition,
    ValueDefinitionSite,
};
use omega_optimization_validation::{
    OptimizationUnitValidationError, OptimizedAbstractPlanProjectionError,
    ValidatedOptimizedAbstractPlanProjection, validate_optimized_abstract_plan_projection,
    validate_psi_rewrite_candidate,
};
use omega_psi_optimizer::{
    OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit, RuleRegistryError,
    baseline_psi_cost_model_identity, built_in_psi_registries,
};
use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractOperationPlan,
    TerminalAbstractParameter,
};
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, LoweringError, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
};
use omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput;
use omega_terminal_target_operations::TerminalTargetOperationPlan;
use psi_core::MachineId;

/// An optimized abstract plan that cannot be constructed without independently
/// replaying its candidates and validating its projection.
///
/// The borrowed plan is executable lowering shape, while the retained run is
/// the evidence that authorizes it. This is not yet native publication
/// authority: target, allocator, machine, byte, and physical provenance gates
/// remain downstream.
#[derive(Debug)]
pub struct ValidatedOptimizedAbstractPlan {
    run: OptimizationRun,
    plan: TerminalAbstractOperationPlan,
    validation: ValidatedOptimizedAbstractPlanProjection,
}

/// Clean target lowering paired with the complete optimized abstract custody
/// that authorized it. No consuming accessor detaches either plan.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    optimized: ValidatedOptimizedAbstractPlan,
    target: omega_target::NativeTarget,
    target_operations: TerminalTargetOperationPlan,
}

impl ValidatedOptimizedTargetOperations {
    pub const fn optimized(&self) -> &ValidatedOptimizedAbstractPlan {
        &self.optimized
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn target_operations(&self) -> &TerminalTargetOperationPlan {
        &self.target_operations
    }
}

pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: omega_target::NativeTarget,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations = lower_to_target_operations(optimized.plan(), target)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target,
        target_operations,
    })
}

pub fn lower_optimized_to_target_operations_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: omega_target::NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations =
        lower_to_target_operations_with_provider_executions(optimized.plan(), target, settlements)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target,
        target_operations,
    })
}

impl ValidatedOptimizedAbstractPlan {
    pub const fn plan(&self) -> &TerminalAbstractOperationPlan {
        &self.plan
    }

    pub const fn verified_input(&self) -> &VerifiedTerminalOptimizationInput {
        self.run.session().input()
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        self.run.session().unit()
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        self.run.selections()
    }

    pub const fn budget_per_pass(&self) -> omega_optimization_core::OptimizationWorkBudget {
        self.run.budget_per_pass()
    }

    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        self.run.commits()
    }

    pub const fn usage(&self) -> OptimizationRunUsage {
        self.run.usage()
    }

    pub const fn decisions(&self) -> &BaselineDecisionLog {
        self.run.decisions()
    }

    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        self.run.pass_manifests()
    }

    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        self.run.transformation_ledger()
    }

    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.run.identity_bundle()
    }

    pub const fn validation(&self) -> ValidatedOptimizedAbstractPlanProjection {
        self.validation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractProjectionError {
    Registry(RuleRegistryError),
    FunctionRosterMismatch,
    InvalidFunctionParameter { machine: MachineId, position: usize },
    InvalidBlockParameter { machine: MachineId, position: usize },
    OperationOffsetOverflow(MachineId),
    InitialUnitProjection,
    CandidateReplay(OptimizationUnitValidationError),
    CommitReplayMismatch,
    FinalUnitReplayMismatch,
    LedgerCommitMismatch,
    ManifestUsageMismatch,
    IndependentValidation(OptimizedAbstractPlanProjectionError),
}

impl std::fmt::Display for OptimizedAbstractProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot project optimized abstract plan: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAbstractProjectionError {}

pub fn project_optimization_run(
    run: OptimizationRun,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError> {
    let registries = built_in_psi_registries(run.selections())
        .map_err(OptimizedAbstractProjectionError::Registry)?;
    let ordered_rules = registries
        .iter()
        .flat_map(|registry| registry.contracts())
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizedAbstractProjectionError::CommitReplayMismatch)?;
    replay_commits(&run)?;
    validate_run_records(&run, ordered_rule_set)?;
    let plan = project_plan(run.session().input().plan(), run.session().unit())?;
    let validation = validate_optimized_abstract_plan_projection(
        run.session().input(),
        run.session().unit(),
        &plan,
        run.selections(),
        ordered_rule_set,
        baseline_psi_cost_model_identity(),
        run.decisions(),
        run.pass_manifests(),
        run.transformation_ledger(),
        run.identity_bundle(),
    )
    .map_err(OptimizedAbstractProjectionError::IndependentValidation)?;
    Ok(ValidatedOptimizedAbstractPlan {
        run,
        plan,
        validation,
    })
}

fn replay_commits(run: &OptimizationRun) -> Result<(), OptimizedAbstractProjectionError> {
    let initial = omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        run.session().input().clone(),
        run.session().unit().fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractProjectionError::InitialUnitProjection)?;
    let mut unit = initial.unit().clone();
    for commit in run.commits() {
        let declaration = commit.declaration();
        if declaration.input() != unit.identity
            || declaration.identity() != commit.candidate
            || declaration.rule() != commit.rule
            || declaration.provenance() != commit.provenance
        {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
        let accepted = validate_psi_rewrite_candidate(&unit, declaration)
            .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
        if accepted.candidate() != commit.candidate
            || accepted.validator() != commit.validator
            || accepted.unit().identity != commit.output
            || commit.input != unit.identity
        {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
        unit = accepted.into_unit();
    }
    if unit != *run.session().unit() {
        return Err(OptimizedAbstractProjectionError::FinalUnitReplayMismatch);
    }
    Ok(())
}

fn validate_run_records(
    run: &OptimizationRun,
    expected_rule_set: OptimizationRuleSetIdentity,
) -> Result<(), OptimizedAbstractProjectionError> {
    let expected_records = run
        .commits()
        .iter()
        .map(|commit| PsiTransformationRecord {
            rule: commit.rule,
            candidate: commit.candidate,
            validator: commit.validator,
            input: commit.input,
            output: commit.output,
            provenance: commit.provenance.clone(),
        })
        .collect::<Vec<_>>();
    if run.transformation_ledger().records() != expected_records {
        return Err(OptimizedAbstractProjectionError::LedgerCommitMismatch);
    }
    let flattened_rules = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    if OptimizationRuleSetIdentity::from_ordered_rules(&flattened_rules).ok()
        != Some(expected_rule_set)
    {
        return Err(OptimizedAbstractProjectionError::ManifestUsageMismatch);
    }
    let mut manifest_usage = OptimizationWorkUsage::default();
    for manifest in run.pass_manifests() {
        manifest_usage = add_work_usage(manifest_usage, manifest.work_usage())
            .ok_or(OptimizedAbstractProjectionError::ManifestUsageMismatch)?;
        OptimizationPassManifestRecord::decode(&manifest.encode())
            .map_err(|_| OptimizedAbstractProjectionError::ManifestUsageMismatch)?;
    }
    if manifest_usage != work_usage(run.usage()) {
        return Err(OptimizedAbstractProjectionError::ManifestUsageMismatch);
    }
    Ok(())
}

fn add_work_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Option<OptimizationWorkUsage> {
    Some(OptimizationWorkUsage {
        rule_evaluations: left.rule_evaluations.checked_add(right.rule_evaluations)?,
        candidates: left.candidates.checked_add(right.candidates)?,
        validation_steps: left.validation_steps.checked_add(right.validation_steps)?,
        commits: left.commits.checked_add(right.commits)?,
        iterations: left.iterations.checked_add(right.iterations)?,
    })
}

const fn work_usage(usage: OptimizationRunUsage) -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: usage.rule_evaluations,
        candidates: usage.candidates,
        validation_steps: usage.validation_steps,
        commits: usage.commits,
        iterations: usage.iterations,
    }
}

fn project_plan(
    source: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<TerminalAbstractOperationPlan, OptimizedAbstractProjectionError> {
    if source.functions.len() != unit.functions.len()
        || source
            .functions
            .iter()
            .map(|function| function.machine)
            .ne(unit.functions.iter().map(|function| function.machine))
    {
        return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
    }
    let functions = source
        .functions
        .iter()
        .zip(&unit.functions)
        .map(|(source, unit)| project_function(source, unit))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalAbstractOperationPlan {
        terminal_psi: source.terminal_psi,
        entry: unit.entry,
        structural_types: source.structural_types.clone(),
        boundary_machines: source.boundary_machines.clone(),
        provider_candidates: source.provider_candidates.clone(),
        functions,
    })
}

fn project_function(
    source: &TerminalAbstractFunction,
    unit: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<TerminalAbstractFunction, OptimizedAbstractProjectionError> {
    let parameters = unit
        .parameters
        .iter()
        .enumerate()
        .map(|(position, definition)| {
            project_parameter(
                definition,
                ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(|_| {
                    OptimizedAbstractProjectionError::InvalidFunctionParameter {
                        machine: unit.machine,
                        position,
                    }
                })?),
                OptimizedAbstractProjectionError::InvalidFunctionParameter {
                    machine: unit.machine,
                    position,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut operation_offset = 0usize;
    let mut block_entries = Vec::with_capacity(unit.blocks.len());
    let mut operations = Vec::new();
    for block in &unit.blocks {
        let parameters = block
            .parameters
            .iter()
            .enumerate()
            .map(|(position, definition)| {
                project_parameter(
                    definition,
                    ValueDefinitionSite::BlockParameter {
                        block: block.id,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizedAbstractProjectionError::InvalidBlockParameter {
                                machine: unit.machine,
                                position,
                            }
                        })?,
                    },
                    OptimizedAbstractProjectionError::InvalidBlockParameter {
                        machine: unit.machine,
                        position,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        block_entries.push(TerminalAbstractBlockEntry {
            block: block.id,
            parameters,
            operation_offset,
        });
        operation_offset = operation_offset.checked_add(block.nodes.len()).ok_or(
            OptimizedAbstractProjectionError::OperationOffsetOverflow(unit.machine),
        )?;
        operations.extend(block.nodes.iter().map(|node| node.operation.clone()));
    }
    Ok(TerminalAbstractFunction {
        machine: unit.machine,
        attachment: source.attachment,
        entry: unit.entry,
        parameters,
        structural_parameters: source.structural_parameters.clone(),
        result: source.result.clone(),
        entry_claims: source.entry_claims.clone(),
        published_service_ceiling: source.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}

fn project_parameter(
    definition: &ValueDefinition,
    expected_site: ValueDefinitionSite,
    error: OptimizedAbstractProjectionError,
) -> Result<TerminalAbstractParameter, OptimizedAbstractProjectionError> {
    if definition.site != expected_site {
        return Err(error);
    }
    Ok(TerminalAbstractParameter {
        value: definition.value,
        scalar_type: definition.scalar_type,
    })
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
    use omega_psi_optimizer::{built_in_psi_registry, run_psi_pipeline};
    use omega_target::NativeTarget;
    use omega_terminal_abstract_operations::TerminalAbstractOperation;
    use omega_terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    use super::*;

    fn work_budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(128, 128, 128, 128, 16).unwrap()
    }

    fn verified(module: TerminalModule, proof: ProofBundle) -> VerifiedPsiOptimizationUnit {
        let semantic = psi_terminal_codec::encode_module(&module).unwrap();
        let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
            )
            .unwrap();
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap()
    }

    fn module_with_blocks(
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

    fn empty_verified() -> VerifiedPsiOptimizationUnit {
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

    fn exact_add_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_011).unwrap();
        let block = BlockId::new(1_012).unwrap();
        let left = ValueId::new(1_013).unwrap();
        let right = ValueId::new(1_014).unwrap();
        let computed = ValueId::new(1_015).unwrap();
        let result = ValueId::new(1_016).unwrap();
        let obligation = ObligationId::new(1_017).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        verified(
            module_with_blocks(
                machine,
                block,
                TerminalMachineResult::Scalar(declaration(result)),
                vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: OperationId::new(1_018).unwrap(),
                            result: OperationResult::Scalar(declaration(left)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            id: OperationId::new(1_019).unwrap(),
                            result: OperationResult::Scalar(declaration(right)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(8),
                            },
                        },
                        Operation {
                            id: OperationId::new(1_020).unwrap(),
                            result: OperationResult::Scalar(declaration(computed)),
                            kind: OperationKind::ExactIntegerAdd {
                                left,
                                right,
                                obligation,
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(1_021).unwrap(),
                        value: computed,
                    },
                }],
            ),
            ProofBundle {
                evidence_producers: Vec::new(),
                evidence: vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                }],
            },
        )
    }

    fn redundant_block_parameter_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_031).unwrap();
        let entry = BlockId::new(1_032).unwrap();
        let exit = BlockId::new(1_033).unwrap();
        let constant = ValueId::new(1_034).unwrap();
        let forwarded = ValueId::new(1_035).unwrap();
        let result = ValueId::new(1_036).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        verified(
            module_with_blocks(
                machine,
                entry,
                TerminalMachineResult::Scalar(declaration(result)),
                vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(1_037).unwrap(),
                            result: OperationResult::Scalar(declaration(constant)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(9),
                            },
                        }],
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(1_038).unwrap(),
                            target: exit,
                            arguments: vec![constant],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: exit,
                        parameters: vec![declaration(forwarded)],
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: EdgeId::new(1_039).unwrap(),
                            value: forwarded,
                        },
                    },
                ],
            ),
            ProofBundle::default(),
        )
    }

    fn run(
        verified: VerifiedPsiOptimizationUnit,
        selections: OptimizationSelections,
    ) -> OptimizationRun {
        let registry = built_in_psi_registry(&selections).unwrap();
        omega_psi_optimizer::run_psi_registry(verified, &selections, &registry, work_budget())
            .unwrap()
    }

    fn run_pipeline(
        verified: VerifiedPsiOptimizationUnit,
        selections: OptimizationSelections,
    ) -> OptimizationRun {
        run_psi_pipeline(verified, &selections, work_budget()).unwrap()
    }

    #[test]
    fn empty_selection_projects_the_original_plan_deterministically() {
        let selections = OptimizationSelections::new([]).unwrap();
        let first = project_optimization_run(run(empty_verified(), selections.clone())).unwrap();
        let second = project_optimization_run(run(empty_verified(), selections)).unwrap();

        assert_eq!(first.plan(), first.verified_input().plan());
        assert_eq!(first.plan(), second.plan());
        assert_eq!(first.validation(), second.validation());
        assert!(first.commits().is_empty());
        assert!(first.pass_manifests().is_empty());
    }

    #[test]
    fn proof_certified_exact_fold_projects_and_remains_target_lowerable() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let optimized = project_optimization_run(run(exact_add_verified(), selections)).unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        let target =
            lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
        assert_eq!(target.target(), NativeTarget::linux_x64());
        assert_eq!(target.optimized().commits().len(), 1);
        assert_eq!(target.target_operations().functions.len(), 1);
    }

    #[test]
    fn copy_propagation_projects_shortened_blocks_and_rewritten_edges() {
        let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let optimized =
            project_optimization_run(run(redundant_block_parameter_verified(), selections))
                .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert!(
            optimized.plan().functions[0].block_entries[1]
                .parameters
                .is_empty()
        );
        assert!(matches!(
            &optimized.plan().functions[0].operations[1],
            TerminalAbstractOperation::Jump { bindings, .. } if bindings.is_empty()
        ));
        assert!(matches!(
            &optimized.plan().functions[0].operations[2],
            TerminalAbstractOperation::Return { value, .. } if *value == ValueId::new(1_034).unwrap()
        ));
        let target =
            lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
        assert_eq!(target.optimized().commits().len(), 1);
    }

    #[test]
    fn independent_validation_rejects_projected_operation_corruption() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let optimized = project_optimization_run(run(exact_add_verified(), selections)).unwrap();
        let mut corrupted = optimized.plan().clone();
        let TerminalAbstractOperation::IntegerConstant { value, .. } =
            &mut corrupted.functions[0].operations[2]
        else {
            panic!("folded operation must be a constant")
        };
        *value = IntegerValue::Unsigned(16);
        let registry = built_in_psi_registry(optimized.selections()).unwrap();

        assert_eq!(
            validate_optimized_abstract_plan_projection(
                optimized.verified_input(),
                optimized.unit(),
                &corrupted,
                optimized.selections(),
                registry.identity(),
                baseline_psi_cost_model_identity(),
                optimized.decisions(),
                optimized.pass_manifests(),
                optimized.transformation_ledger(),
                optimized.identity_bundle(),
            ),
            Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)
        );
    }

    #[test]
    fn independent_validation_rejects_block_offset_corruption() {
        let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let optimized =
            project_optimization_run(run(redundant_block_parameter_verified(), selections))
                .unwrap();
        let mut corrupted = optimized.plan().clone();
        corrupted.functions[0].block_entries[1].operation_offset += 1;
        let registry = built_in_psi_registry(optimized.selections()).unwrap();

        assert_eq!(
            validate_optimized_abstract_plan_projection(
                optimized.verified_input(),
                optimized.unit(),
                &corrupted,
                optimized.selections(),
                registry.identity(),
                baseline_psi_cost_model_identity(),
                optimized.decisions(),
                optimized.pass_manifests(),
                optimized.transformation_ledger(),
                optimized.identity_bundle(),
            ),
            Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)
        );
    }

    #[test]
    fn candidate_replay_rejects_corrupted_commit_custody() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let mut run = run(exact_add_verified(), selections);
        run.commits[0].input = run.commits[0].output;

        assert!(matches!(
            project_optimization_run(run),
            Err(OptimizedAbstractProjectionError::CommitReplayMismatch)
        ));
    }

    #[test]
    fn multi_pass_projection_retains_zero_commit_manifest_in_canonical_order() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let optimized =
            project_optimization_run(run_pipeline(exact_add_verified(), selections)).unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 2);
        assert_eq!(optimized.pass_manifests()[0].work_usage().commits, 1);
        assert_eq!(optimized.pass_manifests()[1].work_usage().commits, 0);
    }

    #[test]
    fn multi_pass_projection_rejects_reordered_or_omitted_manifests() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let mut reordered = run_pipeline(exact_add_verified(), selections.clone());
        reordered.pass_manifests.swap(0, 1);
        assert!(matches!(
            project_optimization_run(reordered),
            Err(OptimizedAbstractProjectionError::ManifestUsageMismatch)
        ));

        let mut omitted = run_pipeline(exact_add_verified(), selections);
        omitted.pass_manifests.pop();
        assert!(matches!(
            project_optimization_run(omitted),
            Err(OptimizedAbstractProjectionError::ManifestUsageMismatch)
        ));
    }
}
