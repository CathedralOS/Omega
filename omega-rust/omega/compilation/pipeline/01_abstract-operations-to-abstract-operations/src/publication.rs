/*
Custody-preserving publication (`publish_optimization_run`,
`ValidatedOptimizedAbstractPlan`, `OptimizedAbstractProjectionError`) from a
completed `OptimizationRun` to an executable `AbstractOperationPlan` and its
sealed pre-physical optimization manifest.
*/

pub(crate) mod error {
    use crate::validation::PrePhysicalOptimizationManifestError;
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedAbstractProjectionError {
        FunctionRosterMismatch,
        InvalidFunctionParameter { machine: MachineId, position: usize },
        InvalidBlockParameter { machine: MachineId, position: usize },
        OperationOffsetOverflow(MachineId),
        PsiSelectionProjectionMismatch,
        PrePhysicalManifest(PrePhysicalOptimizationManifestError),
    }

    failed_display!(
        OptimizedAbstractProjectionError,
        "cannot project optimized abstract plan"
    );
}
pub(crate) mod source {
    pub(crate) mod function {
        use abstract_operations::{AbstractBlockEntry, AbstractFunction};
        use optimization_unit::{PsiOptimizationFunction, ValueDefinitionSite};

        use super::project_parameter;
        use crate::OptimizedAbstractProjectionError;

        pub(crate) fn project(
            unit: &PsiOptimizationFunction,
        ) -> Result<AbstractFunction, OptimizedAbstractProjectionError> {
            let parameters = unit
                .parameters
                .iter()
                .enumerate()
                .map(|(position, definition)| {
                    project_parameter(
                        definition,
                        ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(
                            |_| OptimizedAbstractProjectionError::InvalidFunctionParameter {
                                machine: unit.machine,
                                position,
                            },
                        )?),
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
                    structural_parameters: block.structural_parameters.clone(),
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
                nominal_cleanup_subject: unit.nominal_cleanup_subject,
                entry: unit.entry,
                parameters,
                structural_parameters: unit.structural_parameters.clone(),
                result: unit.result.clone(),
                entry_claims: unit.entry_claim_declarations.clone(),
                block_entry_claims: unit.block_entry_claims.clone(),
                published_service_ceiling: unit.published_service_ceiling.clone(),
                block_entries,
                operations,
            })
        }
    }

    use abstract_operations::{AbstractOperationPlan, AbstractParameter};
    use optimization_unit::{PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite};

    use crate::OptimizedAbstractProjectionError;

    pub(crate) fn project_plan(
        source: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<AbstractOperationPlan, OptimizedAbstractProjectionError> {
        validate_function_roster(source, unit)?;
        let functions = unit
            .functions
            .iter()
            .map(function::project)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AbstractOperationPlan {
            psi: unit.psi,
            entry: unit.entry,
            structural_types: unit.structural_types.clone(),
            boundary_machines: unit.boundary_machines.clone(),
            provider_candidates: unit.provider_candidates.clone(),
            dynamic_dispatch: unit.dynamic_dispatch.clone(),
            identities: unit.identities.clone(),
            functions,
        })
    }

    fn validate_function_roster(
        source: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), OptimizedAbstractProjectionError> {
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
        Ok(())
    }

    pub(crate) fn project_parameter(
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
}

use crate::OptimizationRun;
use optimization_core::OptimizationExecutionPhase;

use crate::validation::{
    project_pre_physical_optimization_manifest, seal_optimized_abstract_plan_projection,
};

use crate::validation::{
    ValidatedOptimizedAbstractPlanProjection, ValidatedPrePhysicalOptimizationManifest,
};
use crate::{OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration};
use abstract_operations::AbstractOperationPlan;
pub(crate) use error::OptimizedAbstractProjectionError;
use optimization_core::{
    BaselineDecisionLog, ExternalDecisionLog, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationSelections,
};
use optimization_unit::{
    AbstractOptimizationEvidence, PsiOptimizationUnit, PsiTransformationLedger,
};
use std::sync::Arc;
use verified_optimization_input::VerifiedPsiOptimizationInput;

pub(crate) fn publish_optimization_run(
    run: OptimizationRun,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError> {
    if run.psi_selections() != &run.selections().for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch);
    }
    let plan = source::project_plan(run.session().input().plan(), run.session().unit())?;
    let validation = seal_optimized_abstract_plan_projection(
        run.session().unit(),
        run.selections(),
        run.psi_selections(),
        run.transformation_ledger(),
        run.identity_bundle(),
    );
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
    Ok(ValidatedOptimizedAbstractPlan::new(
        run,
        plan,
        validation,
        pre_physical_manifest,
    ))
}

#[derive(Debug)]
pub struct ValidatedOptimizedAbstractPlan {
    plan: Arc<AbstractOperationPlan>,
    replay_input: VerifiedPsiOptimizationInput,
    replay_unit: PsiOptimizationUnit,
    evidence: AbstractOptimizationEvidence,
    validation: ValidatedOptimizedAbstractPlanProjection,
    pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
}

impl ValidatedOptimizedAbstractPlan {
    fn new(
        run: OptimizationRun,
        plan: AbstractOperationPlan,
        validation: ValidatedOptimizedAbstractPlanProjection,
        pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
    ) -> Self {
        let OptimizationRun {
            session,
            selections,
            psi_selections,
            budget_per_pass,
            commits,
            validated_candidates,
            usage,
            decisions,
            external_decisions,
            pass_manifests,
            transformation_ledger,
            identity_bundle,
        } = run;
        let (replay_input, replay_unit) = session.into_parts();
        Self {
            plan: Arc::new(plan),
            replay_input,
            replay_unit,
            evidence: AbstractOptimizationEvidence {
                selections,
                psi_selections,
                budget_per_pass,
                commits,
                validated_candidates,
                usage,
                decisions,
                external_decisions,
                pass_manifests,
                transformation_ledger,
                identity_bundle,
            },
            validation,
            pre_physical_manifest,
        }
    }

    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }
    pub fn shared_program(&self) -> Arc<AbstractOperationPlan> {
        Arc::clone(&self.plan)
    }
    pub const fn evidence(&self) -> &AbstractOptimizationEvidence {
        &self.evidence
    }
    pub const fn verified_input(&self) -> &VerifiedPsiOptimizationInput {
        &self.replay_input
    }
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.replay_unit
    }
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.evidence.selections
    }
    pub const fn psi_selections(&self) -> &OptimizationSelections {
        &self.evidence.psi_selections
    }
    pub const fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.evidence.budget_per_pass
    }
    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        &self.evidence.commits
    }
    pub fn validated_candidates(&self) -> &[PsiValidatedCandidateDeclaration] {
        &self.evidence.validated_candidates
    }
    pub const fn usage(&self) -> OptimizationRunUsage {
        self.evidence.usage
    }
    pub const fn decisions(&self) -> &BaselineDecisionLog {
        &self.evidence.decisions
    }
    pub const fn external_decisions(&self) -> &ExternalDecisionLog {
        &self.evidence.external_decisions
    }
    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        &self.evidence.pass_manifests
    }
    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        &self.evidence.transformation_ledger
    }
    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.evidence.identity_bundle
    }
    pub const fn validation(&self) -> ValidatedOptimizedAbstractPlanProjection {
        self.validation
    }
    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
    }

    pub fn into_lowering_parts(
        self,
    ) -> (
        optimization_unit::OptimizedAbstractProgram,
        provider_admission::AdmittedLoweringFacts,
    ) {
        let program = optimization_unit::OptimizedAbstractProgram {
            plan: self.plan,
            unit: self.replay_unit,
            selections: self.evidence.selections,
            budget_per_pass: self.evidence.budget_per_pass,
            identity_bundle: self.evidence.identity_bundle,
            projection: self.validation.identity(),
            projection_psi: self.validation.psi(),
            final_unit: self.validation.final_unit(),
            pre_physical_manifest: self.pre_physical_manifest.record().clone(),
        };
        (program, self.replay_input.into_lowering_facts())
    }
}

const fn work_usage(usage: OptimizationRunUsage) -> optimization_core::OptimizationWorkUsage {
    optimization_core::OptimizationWorkUsage {
        rule_evaluations: usage.rule_evaluations,
        candidates: usage.candidates,
        validation_steps: usage.validation_steps,
        commits: usage.commits,
        iterations: usage.iterations,
    }
}
