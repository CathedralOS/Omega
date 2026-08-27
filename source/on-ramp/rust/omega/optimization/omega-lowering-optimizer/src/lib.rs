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
    PrePhysicalOptimizationManifestError, ValidatedOptimizedAbstractPlanProjection,
    ValidatedPrePhysicalOptimizationManifest, project_pre_physical_optimization_manifest,
    validate_optimized_abstract_plan_projection, validate_psi_rewrite_candidate,
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
    pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
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

    pub const fn psi_selections(&self) -> &OptimizationSelections {
        self.run.psi_selections()
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

    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
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
    PsiSelectionProjectionMismatch,
    IndependentValidation(OptimizedAbstractPlanProjectionError),
    PrePhysicalManifest(PrePhysicalOptimizationManifestError),
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
    if run.psi_selections()
        != &run
            .selections()
            .for_phase(omega_optimization_core::OptimizationExecutionPhase::Psi)
    {
        return Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch);
    }
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
        run.psi_selections(),
        ordered_rule_set,
        baseline_psi_cost_model_identity(),
        run.decisions(),
        run.pass_manifests(),
        run.transformation_ledger(),
        run.identity_bundle(),
    )
    .map_err(OptimizedAbstractProjectionError::IndependentValidation)?;
    let pre_physical_manifest = project_pre_physical_optimization_manifest(
        run.session().input(),
        run.session().unit(),
        run.selections(),
        run.psi_selections(),
        run.budget_per_pass(),
        work_usage(run.usage()),
        run.decisions(),
        run.pass_manifests(),
        run.transformation_ledger(),
        run.identity_bundle(),
        validation,
    )
    .map_err(OptimizedAbstractProjectionError::PrePhysicalManifest)?;
    Ok(ValidatedOptimizedAbstractPlan {
        run,
        plan,
        validation,
        pre_physical_manifest,
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
            pruned_machines: commit.pruned_machines.clone(),
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
    if source.functions.len() != unit.functions.len() + unit.pruned_machines.len() {
        return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
    }
    let mut active = unit.functions.iter();
    let mut next_active = active.next();
    for (ordinal, source_function) in source.functions.iter().enumerate() {
        if next_active.is_some_and(|function| function.machine == source_function.machine) {
            next_active = active.next();
            continue;
        }
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| OptimizedAbstractProjectionError::FunctionRosterMismatch)?;
        if !unit.pruned_machines.iter().any(|custody| {
            custody.source_ordinal == ordinal && custody.machine == source_function.machine
        }) {
            return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
        }
    }
    if next_active.is_some() {
        return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
    }
    let functions = unit
        .functions
        .iter()
        .map(project_function)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalAbstractOperationPlan {
        terminal_psi: unit.terminal_psi,
        entry: unit.entry,
        structural_types: unit.structural_types.clone(),
        boundary_machines: unit.boundary_machines.clone(),
        provider_candidates: unit.provider_candidates.clone(),
        functions,
    })
}

fn project_function(
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
        attachment: unit.attachment,
        entry: unit.entry,
        parameters,
        structural_parameters: unit.structural_parameters.clone(),
        result: unit.result.clone(),
        entry_claims: unit.entry_claim_declarations.clone(),
        published_service_ceiling: unit.published_service_ceiling.clone(),
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
    use omega_optimization_validation::{
        PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest,
        PrePhysicalOptimizationManifestDecodeError, PrePhysicalOptimizationManifestError,
        validate_pre_physical_optimization_manifest,
    };
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
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
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

    fn dead_scalar_literals_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_081).unwrap();
        let block = BlockId::new(1_082).unwrap();
        let boolean = ValueId::new(1_083).unwrap();
        let integer = ValueId::new(1_084).unwrap();
        verified(
            module_with_blocks(
                machine,
                block,
                TerminalMachineResult::Unit,
                vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: OperationId::new(1_085).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: boolean,
                                scalar_type: ScalarType::Boolean,
                            }),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            id: OperationId::new(1_086).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: integer,
                                scalar_type: ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                                ),
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1_087).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
            ),
            ProofBundle::default(),
        )
    }

    fn dead_wrapping_add_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_091).unwrap();
        let block = BlockId::new(1_092).unwrap();
        let left = ValueId::new(1_093).unwrap();
        let right = ValueId::new(1_094).unwrap();
        let sum = ValueId::new(1_095).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let declaration = |id| {
            OperationResult::Scalar(ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(integer),
            })
        };
        verified(
            module_with_blocks(
                machine,
                block,
                TerminalMachineResult::Unit,
                vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: OperationId::new(1_096).unwrap(),
                            result: declaration(left),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(250),
                            },
                        },
                        Operation {
                            id: OperationId::new(1_097).unwrap(),
                            result: declaration(right),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(10),
                            },
                        },
                        Operation {
                            id: OperationId::new(1_098).unwrap(),
                            result: declaration(sum),
                            kind: OperationKind::WrappingIntegerAdd { left, right },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1_099).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
            ),
            ProofBundle::default(),
        )
    }

    fn local_cse_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_321).unwrap();
        let block = BlockId::new(1_322).unwrap();
        let operand = ValueId::new(1_323).unwrap();
        let leader = ValueId::new(1_324).unwrap();
        let redundant = ValueId::new(1_325).unwrap();
        let result = ValueId::new(1_326).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let mut module = module_with_blocks(
            machine,
            block,
            TerminalMachineResult::Scalar(declaration(result)),
            vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(1_327).unwrap(),
                        result: OperationResult::Scalar(declaration(leader)),
                        kind: OperationKind::IntegerBitwiseNot { operand },
                    },
                    Operation {
                        id: OperationId::new(1_328).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant)),
                        kind: OperationKind::IntegerBitwiseNot { operand },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_329).unwrap(),
                    value: redundant,
                    cleanup_actions: Vec::new(),
                },
            }],
        );
        module.machines[0].parameters.push(declaration(operand));
        verified(module, ProofBundle::default())
    }

    fn dominator_gvn_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_361).unwrap();
        let child = BlockId::new(1_362).unwrap();
        let entry = BlockId::new(1_363).unwrap();
        let operand = ValueId::new(1_364).unwrap();
        let leader = ValueId::new(1_365).unwrap();
        let redundant = ValueId::new(1_366).unwrap();
        let result = ValueId::new(1_367).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let mut module = module_with_blocks(
            machine,
            entry,
            TerminalMachineResult::Scalar(declaration(result)),
            vec![
                Block {
                    id: child,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_368).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant)),
                        kind: OperationKind::IntegerBitwiseNot { operand },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(1_369).unwrap(),
                        value: redundant,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_370).unwrap(),
                        result: OperationResult::Scalar(declaration(leader)),
                        kind: OperationKind::IntegerBitwiseNot { operand },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_371).unwrap(),
                        target: child,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
        );
        module.machines[0].parameters.push(declaration(operand));
        verified(module, ProofBundle::default())
    }

    fn unreachable_private_machine_verified() -> VerifiedPsiOptimizationUnit {
        let entry_machine = MachineId::new(1_041).unwrap();
        let entry_block = BlockId::new(1_042).unwrap();
        let mut module = module_with_blocks(
            entry_machine,
            entry_block,
            TerminalMachineResult::Unit,
            vec![Block {
                id: entry_block,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1_043).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
        );
        let private_machine = MachineId::new(1_044).unwrap();
        let private_block = BlockId::new(1_045).unwrap();
        module.machines.push(TerminalMachine {
            id: private_machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: private_block,
            blocks: vec![Block {
                id: private_block,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1_046).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1_047).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
        verified(module, ProofBundle::default())
    }

    fn adjacent_terminal_jump_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_051).unwrap();
        let entry = BlockId::new(1_052).unwrap();
        let target = BlockId::new(1_053).unwrap();
        verified(
            module_with_blocks(
                machine,
                entry,
                TerminalMachineResult::Unit,
                vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(1_054).unwrap(),
                            target,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: target,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::ReturnUnit {
                            edge: EdgeId::new(1_055).unwrap(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                ],
            ),
            ProofBundle::default(),
        )
    }

    fn non_adjacent_block_merge_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_501).unwrap();
        let entry = BlockId::new(1_502).unwrap();
        let descendant = BlockId::new(1_503).unwrap();
        let target = BlockId::new(1_504).unwrap();
        let sibling = BlockId::new(1_505).unwrap();
        let predecessor = BlockId::new(1_506).unwrap();
        let condition = ValueId::new(1_507).unwrap();
        let incoming = ValueId::new(1_508).unwrap();
        let target_parameter = ValueId::new(1_509).unwrap();
        let target_result = ValueId::new(1_510).unwrap();
        let computed = ValueId::new(1_511).unwrap();
        let predecessor_value = ValueId::new(1_520).unwrap();
        let result = ValueId::new(1_522).unwrap();
        let boolean = |id| ValueDeclaration {
            id,
            scalar_type: ScalarType::Boolean,
        };
        let mut module = module_with_blocks(
            machine,
            entry,
            TerminalMachineResult::Scalar(boolean(result)),
            vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(1_512).unwrap(),
                            target: predecessor,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(1_513).unwrap(),
                            target: sibling,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: descendant,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_514).unwrap(),
                        result: OperationResult::Scalar(boolean(computed)),
                        kind: OperationKind::BooleanEqual {
                            left: target_parameter,
                            right: target_result,
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(1_515).unwrap(),
                        value: computed,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: vec![boolean(target_parameter)],
                    operations: vec![Operation {
                        id: OperationId::new(1_516).unwrap(),
                        result: OperationResult::Scalar(boolean(target_result)),
                        kind: OperationKind::BooleanNot {
                            operand: target_parameter,
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_517).unwrap(),
                        target: descendant,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: sibling,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(1_518).unwrap(),
                        value: incoming,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: predecessor,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_521).unwrap(),
                        result: OperationResult::Scalar(boolean(predecessor_value)),
                        kind: OperationKind::BooleanNot { operand: incoming },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_519).unwrap(),
                        target,
                        arguments: vec![predecessor_value],
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
        );
        module.machines[0]
            .parameters
            .extend([boolean(condition), boolean(incoming)]);
        verified(module, ProofBundle::default())
    }

    fn shared_terminal_jump_verified() -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_061).unwrap();
        let entry = BlockId::new(1_062).unwrap();
        let left = BlockId::new(1_063).unwrap();
        let right = BlockId::new(1_064).unwrap();
        let target = BlockId::new(1_065).unwrap();
        let condition = ValueId::new(1_066).unwrap();
        let left_value = ValueId::new(1_067).unwrap();
        let right_value = ValueId::new(1_068).unwrap();
        let boolean = |id| ValueDeclaration {
            id,
            scalar_type: ScalarType::Boolean,
        };
        let mut module = module_with_blocks(
            machine,
            entry,
            TerminalMachineResult::Unit,
            vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(1_069).unwrap(),
                            target: left,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(1_070).unwrap(),
                            target: right,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: left,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_071).unwrap(),
                        result: OperationResult::Scalar(boolean(left_value)),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_072).unwrap(),
                        target,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: right,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_073).unwrap(),
                        result: OperationResult::Scalar(boolean(right_value)),
                        kind: OperationKind::BooleanConstant { value: false },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_074).unwrap(),
                        target,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1_075).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
        );
        module.machines[0].parameters.push(boolean(condition));
        verified(module, ProofBundle::default())
    }

    fn exact_add_verified() -> VerifiedPsiOptimizationUnit {
        exact_add_verified_with_result(true)
    }

    fn dead_exact_add_verified() -> VerifiedPsiOptimizationUnit {
        exact_add_verified_with_result(false)
    }

    fn exact_add_verified_with_result(return_result: bool) -> VerifiedPsiOptimizationUnit {
        let machine = MachineId::new(1_011).unwrap();
        let block = BlockId::new(1_012).unwrap();
        let left = ValueId::new(1_013).unwrap();
        let right = ValueId::new(1_014).unwrap();
        let computed = ValueId::new(1_015).unwrap();
        let result = ValueId::new(1_016).unwrap();
        let obligation = ObligationId::new(1_017).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let machine_result = if return_result {
            TerminalMachineResult::Scalar(declaration(result))
        } else {
            TerminalMachineResult::Unit
        };
        let terminator = if return_result {
            Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_021).unwrap(),
                value: computed,
            }
        } else {
            Terminator::ReturnUnit {
                edge: EdgeId::new(1_021).unwrap(),
                trivial_affine_discards: Vec::new(),
            }
        };
        verified(
            module_with_blocks(
                machine,
                block,
                machine_result,
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
                    terminator,
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
    fn private_machine_pruning_projects_exact_roster_and_ledger_custody() {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let optimized =
            project_optimization_run(run(unreachable_private_machine_verified(), selections))
                .unwrap();

        assert_eq!(optimized.verified_input().plan().functions.len(), 2);
        assert_eq!(optimized.plan().functions.len(), 1);
        assert_eq!(optimized.unit().functions.len(), 1);
        assert_eq!(optimized.unit().pruned_machines.len(), 1);
        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(
            optimized.transformation_ledger().records()[0].pruned_machines,
            optimized.unit().pruned_machines
        );
        assert!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .iter()
                .all(|row| matches!(
                    row.disposition,
                    omega_optimization_unit::ProvenanceDisposition::ProvenUnreachableAt(_)
                ))
        );

        let mut wrong_ordinal = optimized.unit().clone();
        wrong_ordinal.pruned_machines[0].source_ordinal = 0;
        wrong_ordinal.identity =
            omega_optimization_unit::recompute_psi_optimization_unit_identity(&wrong_ordinal);
        assert_eq!(
            omega_optimization_validation::validate_transformed_psi_optimization_unit(
                optimized.verified_input(),
                &wrong_ordinal,
            ),
            Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)
        );
    }

    #[test]
    fn adjacent_terminal_jump_fusion_reaches_verified_one_block_projection() {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let optimized =
            project_optimization_run(run(adjacent_terminal_jump_verified(), selections)).unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
        assert_eq!(optimized.unit().functions[0].blocks.len(), 1);
        assert_eq!(
            optimized.unit().functions[0].blocks[0].nodes[0].provenance,
            [
                omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(1_055).unwrap()),
                omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(1_054).unwrap()),
            ]
        );
    }

    #[test]
    fn non_adjacent_block_merges_replay_and_lower_in_both_target_families() {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let optimized =
            project_optimization_run(run(non_adjacent_block_merge_verified(), selections.clone()))
                .unwrap();

        assert_eq!(optimized.commits().len(), 2);
        assert_eq!(optimized.transformation_ledger().records().len(), 2);
        assert_eq!(
            optimized
                .transformation_ledger()
                .records()
                .iter()
                .map(|record| record.provenance.len())
                .collect::<Vec<_>>(),
            [6, 6]
        );
        assert!(
            optimized
                .transformation_ledger()
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .all(|row| row.disposition.is_realized())
        );
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 3);
        assert_eq!(optimized.plan().functions[0].operations.len(), 6);
        assert_eq!(optimized.unit().functions[0].blocks.len(), 3);
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            TerminalAbstractOperation::BooleanNot { .. }
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[3],
            TerminalAbstractOperation::BooleanNot { .. }
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[4],
            TerminalAbstractOperation::BooleanEqual { .. }
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[5],
            TerminalAbstractOperation::Return { .. }
        ));

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let optimized = project_optimization_run(run(
                non_adjacent_block_merge_verified(),
                selections.clone(),
            ))
            .unwrap();
            let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
            assert_eq!(lowered.target(), target);
            assert_eq!(lowered.optimized().commits().len(), 2);
        }
    }

    #[test]
    fn shared_terminal_jump_fusion_replays_to_two_exact_terminal_occurrences() {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let optimized =
            project_optimization_run(run(shared_terminal_jump_verified(), selections)).unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 4);
        assert_eq!(optimized.unit().functions[0].blocks.len(), 4);
        let terminal_source =
            omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(1_075).unwrap());
        let terminal_nodes = optimized.unit().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .filter(|node| node.provenance.contains(&terminal_source))
            .collect::<Vec<_>>();
        assert_eq!(terminal_nodes.len(), 2);
        assert!(
            terminal_nodes
                .iter()
                .all(|node| matches!(node.operation, TerminalAbstractOperation::ReturnUnit { .. }))
        );
        let source_site = omega_optimization_unit::PsiRealizationSite::Node(
            omega_optimization_unit::NodeLocation {
                machine: MachineId::new(1_061).unwrap(),
                block: BlockId::new(1_065).unwrap(),
                node: 0,
            },
        );
        assert_eq!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .iter()
                .filter(|row| row.input == source_site)
                .count(),
            2
        );
    }

    #[test]
    fn dead_scalar_literal_elimination_replays_transitive_fuel_to_the_terminal() {
        let selections =
            OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
        let optimized =
            project_optimization_run(run(dead_scalar_literals_verified(), selections)).unwrap();
        assert_eq!(optimized.commits().len(), 2);
        assert_eq!(optimized.plan().functions[0].operations.len(), 1);
        assert_eq!(optimized.unit().functions[0].facts.len(), 0);
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert!(matches!(
            terminal.operation,
            TerminalAbstractOperation::ReturnUnit { .. }
        ));
        assert_eq!(terminal.provenance.len(), 3);
        assert_eq!(terminal.fuel.len(), 3);
        assert!(
            optimized
                .transformation_ledger()
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .all(|row| row.disposition.is_realized())
        );
    }

    #[test]
    fn dead_scalar_suite_removes_total_arithmetic_then_its_dead_operands() {
        let selections =
            OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
        let optimized =
            project_optimization_run(run(dead_wrapping_add_verified(), selections)).unwrap();
        assert_eq!(optimized.commits().len(), 3);
        assert_eq!(optimized.plan().functions[0].operations.len(), 1);
        assert_eq!(optimized.unit().functions[0].facts.len(), 0);
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert!(matches!(
            terminal.operation,
            TerminalAbstractOperation::ReturnUnit { .. }
        ));
        assert_eq!(terminal.provenance.len(), 4);
        assert_eq!(terminal.fuel.len(), 4);
        assert!(
            optimized
                .transformation_ledger()
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .all(|row| row.disposition.is_realized())
        );
    }

    #[test]
    fn global_value_numbering_projects_local_cse_and_return_substitution() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let optimized = project_optimization_run(run(local_cse_verified(), selections)).unwrap();
        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        assert!(
            matches!(optimized.plan().functions[0].operations[0], TerminalAbstractOperation::IntegerBitwiseNot { result, .. } if result == ValueId::new(1_324).unwrap())
        );
        assert!(
            matches!(optimized.plan().functions[0].operations[1], TerminalAbstractOperation::Return { value, .. } if value == ValueId::new(1_324).unwrap())
        );
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .len(),
            2
        );
    }

    #[test]
    fn global_value_numbering_projects_a_non_roster_order_dominating_leader() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let optimized =
            project_optimization_run(run(dominator_gvn_verified(), selections)).unwrap();
        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(
            optimized.plan().functions[0].block_entries[0].block,
            BlockId::new(1_362).unwrap()
        );
        assert!(
            matches!(optimized.plan().functions[0].operations[0], TerminalAbstractOperation::Return { value, .. } if value == ValueId::new(1_365).unwrap())
        );
        assert!(
            matches!(optimized.plan().functions[0].operations[1], TerminalAbstractOperation::IntegerBitwiseNot { result, .. } if result == ValueId::new(1_365).unwrap())
        );
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .iter()
                .all(|row| row.disposition.is_realized())
        );
    }

    #[test]
    fn lower_only_suite_records_no_psi_completion() {
        let selections =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        let optimized =
            project_optimization_run(run_pipeline(empty_verified(), selections.clone())).unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert!(optimized.psi_selections().is_empty());
        assert_eq!(optimized.plan(), optimized.verified_input().plan());
        assert!(optimized.commits().is_empty());
        assert!(optimized.pass_manifests().is_empty());
        assert_eq!(
            optimized.pre_physical_manifest().record().selections,
            selections
        );
        assert!(
            optimized
                .pre_physical_manifest()
                .record()
                .psi_selections
                .is_empty()
        );
    }

    #[test]
    fn mixed_suite_records_only_its_psi_completion() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let psi_selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let optimized =
            project_optimization_run(run_pipeline(exact_add_verified(), selections.clone()))
                .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert_eq!(optimized.psi_selections(), &psi_selections);
        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(
            optimized.pre_physical_manifest().record().selections,
            selections
        );
        assert_eq!(
            optimized.pre_physical_manifest().record().psi_selections,
            psi_selections
        );
    }

    #[test]
    fn projection_rejects_a_tampered_psi_phase_projection() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let mut run = run_pipeline(exact_add_verified(), selections);
        run.psi_selections = OptimizationSelections::default();

        assert!(matches!(
            project_optimization_run(run),
            Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch)
        ));
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
    fn proof_check_elision_projects_dead_exact_work_and_retains_evidence() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let optimized =
            project_optimization_run(run(dead_exact_add_verified(), selections)).unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            1
        );
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[2];
        assert!(matches!(
            terminal.operation,
            TerminalAbstractOperation::ReturnUnit { .. }
        ));
        assert_eq!(terminal.provenance.len(), 2);
        assert_eq!(terminal.fuel.len(), 2);
    }

    #[test]
    fn pre_physical_manifest_is_deterministic_structured_and_independently_validated() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let first =
            project_optimization_run(run(exact_add_verified(), selections.clone())).unwrap();
        let second = project_optimization_run(run(exact_add_verified(), selections)).unwrap();
        let manifest = first.pre_physical_manifest().record();

        assert_eq!(manifest, second.pre_physical_manifest().record());
        assert_eq!(manifest.identity, manifest.recomputed_identity());
        let encoded = manifest.encode();
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&encoded),
            Ok(manifest.clone())
        );
        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&identity_tamper),
            Err(PrePhysicalOptimizationManifestDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&trailing),
            Err(PrePhysicalOptimizationManifestDecodeError::TrailingBytes)
        );
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&encoded[..encoded.len() - 1]),
            Err(PrePhysicalOptimizationManifestDecodeError::Truncated)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&wrong_magic),
            Err(PrePhysicalOptimizationManifestDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&wrong_version),
            Err(PrePhysicalOptimizationManifestDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            manifest.physical_data,
            PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization
        );
        assert_eq!(manifest.initial_unit, first.transformation_ledger().input());
        assert_eq!(manifest.final_unit, first.unit().identity);
        assert_eq!(manifest.projection, first.validation().identity());
        assert_eq!(manifest.decision_log, *first.decisions());
        assert_eq!(manifest.pass_manifests, first.pass_manifests());
        assert_eq!(
            manifest.transformation_ledger,
            *first.transformation_ledger()
        );
        assert_eq!(manifest.source_statistics.functions, 1);
        assert_eq!(manifest.source_statistics.blocks, 1);
        assert_eq!(manifest.source_statistics.nodes, 4);
        assert_eq!(manifest.optimized_statistics.nodes, 4);
        let text = manifest.render_text();
        assert!(text.contains("SparseConditionalConstantPropagation"));
        assert!(text.contains("physical data: unavailable before physical realization"));
        assert!(text.contains("candidate verdicts: applied=1, skipped=0, rejected=0"));
        assert!(text.contains("fact: accepted-obligation:"));
        assert!(text.contains("source: operation:"));
        assert!(text.contains("source-scheduled-fuel: operation:"));
        assert!(text.contains("runtime-charge=1"));

        let replay = validate_pre_physical_optimization_manifest(
            manifest,
            first.verified_input(),
            first.unit(),
            first.selections(),
            first.psi_selections(),
            first.budget_per_pass(),
            work_usage(first.usage()),
            first.decisions(),
            first.pass_manifests(),
            first.transformation_ledger(),
            first.identity_bundle(),
            first.validation(),
        )
        .unwrap();
        assert_eq!(replay, *first.pre_physical_manifest());

        let mut corrupted = manifest.clone();
        corrupted.optimized_statistics.nodes += 1;
        corrupted.identity = corrupted.recomputed_identity();
        assert_eq!(
            validate_pre_physical_optimization_manifest(
                &corrupted,
                first.verified_input(),
                first.unit(),
                first.selections(),
                first.psi_selections(),
                first.budget_per_pass(),
                work_usage(first.usage()),
                first.decisions(),
                first.pass_manifests(),
                first.transformation_ledger(),
                first.identity_bundle(),
                first.validation(),
            ),
            Err(PrePhysicalOptimizationManifestError::ContentMismatch)
        );

        let mut omitted_pass = manifest.clone();
        omitted_pass.pass_manifests.clear();
        omitted_pass.identity = omitted_pass.recomputed_identity();
        assert_eq!(
            validate_pre_physical_optimization_manifest(
                &omitted_pass,
                first.verified_input(),
                first.unit(),
                first.selections(),
                first.psi_selections(),
                first.budget_per_pass(),
                work_usage(first.usage()),
                first.decisions(),
                first.pass_manifests(),
                first.transformation_ledger(),
                first.identity_bundle(),
                first.validation(),
            ),
            Err(PrePhysicalOptimizationManifestError::ContentMismatch)
        );

        let mut wrong_selections = manifest.clone();
        wrong_selections.selections =
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        wrong_selections.identity = wrong_selections.recomputed_identity();
        assert_eq!(
            validate_pre_physical_optimization_manifest(
                &wrong_selections,
                first.verified_input(),
                first.unit(),
                first.selections(),
                first.psi_selections(),
                first.budget_per_pass(),
                work_usage(first.usage()),
                first.decisions(),
                first.pass_manifests(),
                first.transformation_ledger(),
                first.identity_bundle(),
                first.validation(),
            ),
            Err(PrePhysicalOptimizationManifestError::ContentMismatch)
        );
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
                optimized.psi_selections(),
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
                optimized.psi_selections(),
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
