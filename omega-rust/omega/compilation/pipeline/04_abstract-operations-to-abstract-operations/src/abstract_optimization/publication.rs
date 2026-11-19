/*
Custody-preserving publication (`publish_optimization_run`,
`ValidatedOptimizedAbstractPlan`, `OptimizedAbstractProjectionError`) from a
completed `OptimizationRun` to an executable `AbstractOperationPlan` and its
sealed pre-physical optimization manifest.
*/

pub(crate) mod error {
    use crate::abstract_optimization::validation::PrePhysicalOptimizationManifestError;
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedAbstractProjectionError {
        FunctionRosterMismatch,
        InvalidFunctionParameter { machine: MachineId, position: usize },
        InvalidBlockParameter { machine: MachineId, position: usize },
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
        use abstract_operations::{AbstractBlock, AbstractFunction};
        use optimization_unit::{PsiOptimizationFunction, ValueDefinitionSite};

        use super::project_parameter;
        use crate::abstract_optimization::OptimizedAbstractProjectionError;

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
            let mut blocks = Vec::with_capacity(unit.blocks.len());
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
                blocks.push(AbstractBlock {
                    structural_parameters: block.structural_parameters.clone(),
                    block: block.id,
                    parameters,
                    operations: block
                        .nodes
                        .iter()
                        .map(|node| node.operation.clone())
                        .collect(),
                });
            }
            Ok(AbstractFunction {
                machine: unit.machine,
                attachment: unit.attachment,
                entry: unit.entry,
                parameters,
                structural_parameters: unit.structural_parameters.clone(),
                result: unit.result.clone(),
                entry_claims: unit.entry_claim_declarations.clone(),
                block_entry_claims: unit.block_entry_claims.clone(),
                published_service_ceiling: unit.published_service_ceiling.clone(),
                blocks,
            })
        }
    }

    use abstract_operations::{AbstractOperationPlan, AbstractParameter};
    use optimization_unit::{PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite};

    use crate::abstract_optimization::OptimizedAbstractProjectionError;

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
            callables: unit.callables.clone(),
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

use crate::abstract_optimization::OptimizationRun;
use optimization_core::OptimizationExecutionPhase;

use crate::abstract_optimization::validation::{
    project_pre_physical_optimization_manifest, seal_optimized_abstract_plan_projection,
};

use crate::abstract_optimization::OptimizationRunUsage;
use crate::abstract_optimization::validation::{
    ValidatedOptimizedAbstractPlanProjection, ValidatedPrePhysicalOptimizationManifest,
};
use abstract_operations::AbstractOperationPlan;
pub(crate) use error::OptimizedAbstractProjectionError;
use optimization_unit::{AbstractOptimizationEvidence, PsiOptimizationUnit};
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
    pub const fn verified_input(&self) -> &VerifiedPsiOptimizationInput {
        &self.replay_input
    }

    pub fn into_lowering_input(self) -> verified_optimization_input::OptimizedAbstractBody {
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
        self.replay_input.into_optimized_body(program)
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
