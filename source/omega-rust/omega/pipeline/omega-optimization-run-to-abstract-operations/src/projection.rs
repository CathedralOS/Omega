use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractOperationPlan, AbstractParameter,
};
use omega_optimization_core::{
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationRuleSetIdentity,
    OptimizationSelections, OptimizationWorkUsage,
};
use omega_optimization_policy::{BaselineDecisionLog, ExternalDecisionLog};
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
    validate_external_decision_recording,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput;
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
    plan: AbstractOperationPlan,
    validation: ValidatedOptimizedAbstractPlanProjection,
    pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
}

impl ValidatedOptimizedAbstractPlan {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn verified_input(&self) -> &VerifiedPsiOptimizationInput {
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

    pub const fn external_decisions(&self) -> &ExternalDecisionLog {
        self.run.external_decisions()
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
    ExternalDecisionRecordingMismatch,
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
    validate_external_decision_recording(&run)
        .map_err(|_| OptimizedAbstractProjectionError::ExternalDecisionRecordingMismatch)?;
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
    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
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
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<AbstractOperationPlan, OptimizedAbstractProjectionError> {
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
    Ok(AbstractOperationPlan {
        psi: unit.psi,
        entry: unit.entry,
        structural_types: unit.structural_types.clone(),
        boundary_machines: unit.boundary_machines.clone(),
        provider_candidates: unit.provider_candidates.clone(),
        functions,
    })
}

fn project_function(
    unit: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<AbstractFunction, OptimizedAbstractProjectionError> {
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
        block_entries.push(AbstractBlockEntry {
            block: block.id,
            parameters,
            operation_offset,
        });
        operation_offset = operation_offset.checked_add(block.nodes.len()).ok_or(
            OptimizedAbstractProjectionError::OperationOffsetOverflow(unit.machine),
        )?;
        operations.extend(block.nodes.iter().map(|node| node.operation.clone()));
    }
    Ok(AbstractFunction {
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
) -> Result<AbstractParameter, OptimizedAbstractProjectionError> {
    if definition.site != expected_site {
        return Err(error);
    }
    Ok(AbstractParameter {
        value: definition.value,
        scalar_type: definition.scalar_type,
    })
}

#[cfg(test)]
mod tests;
