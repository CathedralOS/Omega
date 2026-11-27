/*
Publication (`publish_optimization_run`, `ValidatedOptimizedAbstractPlan`,
`OptimizedAbstractProjectionError`) of the identity `OptimizationRun` as the
input `AbstractOperationPlan` with its sealed projection and pre-physical
optimization manifest.
*/

pub(crate) mod error {
    use crate::abstract_optimization::validation::PrePhysicalOptimizationManifestError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedAbstractProjectionError {
        PsiSelectionProjectionMismatch,
        PrePhysicalManifest(PrePhysicalOptimizationManifestError),
    }

    failed_display!(
        OptimizedAbstractProjectionError,
        "cannot project optimized abstract plan"
    );
}
use crate::abstract_optimization::OptimizationRun;
use optimization_core::OptimizationExecutionPhase;

use crate::abstract_optimization::validation::{
    InitialUnitRecord, project_pre_physical_optimization_manifest,
    seal_optimized_abstract_plan_projection,
};

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
    initial: InitialUnitRecord,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError> {
    if run.psi_selections() != &run.selections().for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch);
    }
    // The identity run leaves the unit as stage 03 built it from this plan,
    // so the input plan is the published plan.
    let plan = run.session().input().plan().clone();
    let validation = seal_optimized_abstract_plan_projection(&run);
    let pre_physical_manifest =
        project_pre_physical_optimization_manifest(&run, initial, validation)
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
