//! Optimizer module role: executable entrance. The complete abstract X-to-X
//! phase, and the run and replay entries it is built from.
//!
//! [`optimize_abstract_operations`] is the phase: admit the verified input
//! and build the optimization unit under the current fuel schedule
//! (`terminal_psi_to_abstract_operations`), run the selected passes through
//! [`run_psi_pipeline_for_projection`], and publish the validated plan
//! through `publication`. Each stage's refusal is one variant of
//! [`AbstractOptimizationError`].
//!
//! The run entries below share one shape: check the selection against the
//! registries it names, open a [`VerifiedPsiOptimizationSession`] (the
//! initial independent validation), then hand the session to
//! `pass_manager::run_registries`, which executes each registry's rules to a
//! fixed point under the budget and records every decision.
//! [`run_psi_pipeline`] and [`run_psi_pipeline_for_projection`] run every
//! selected built-in registry in schedule order; [`run_psi_registry`] runs
//! one registry the caller constructed, refusing one that is not the exact
//! built-in registry for the selection. [`replay_psi_pipeline`] and
//! [`replay_psi_registry`] are the same runs with the action after each
//! candidate's validation taken from a canonical external decision log,
//! decoded strictly at this byte boundary.

use optimization_core::{
    ExternalDecisionLog, OptimizationSelections, OptimizationWorkBudget,
    PsiOptimizationSelectionProjection,
};
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};

use crate::pass_manager::{
    ExternalDecisionReplayError, OptimizationRun, OptimizationRunError,
    VerifiedPsiOptimizationSession, run_registries, run_registries_with_external_decisions,
};
use crate::publication::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, publish_optimization_run,
};
use crate::rules::registry::OrderedRuleRegistry;
use crate::rules::{
    built_in_psi_registries, built_in_psi_registries_for_selections, built_in_psi_registry,
};

#[derive(Debug)]
pub enum AbstractOptimizationError {
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    Publication(OptimizedAbstractProjectionError),
}

impl std::fmt::Display for AbstractOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "abstract optimization failed: {self:?}")
    }
}

impl std::error::Error for AbstractOptimizationError {}

/// The phase: build the verified unit, run the selected passes, publish.
pub fn optimize_abstract_operations(
    input: VerifiedPsiOptimizationInput,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<ValidatedOptimizedAbstractPlan, AbstractOptimizationError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(AbstractOptimizationError::UnitBuild)?;
    let run =
        run_psi_pipeline_for_projection(verified, selections, psi_selections, budget_per_pass)
            .map_err(AbstractOptimizationError::Run)?;
    publish_optimization_run(run).map_err(AbstractOptimizationError::Publication)
}

pub fn run_psi_registry(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let projection = selections.project_psi();
    let expected =
        built_in_psi_registry(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    if expected.identity() != registry.identity()
        || expected.contracts().collect::<Vec<_>>() != registry.contracts().collect::<Vec<_>>()
    {
        return Err(OptimizationRunError::SelectionRegistryMismatch);
    }
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    if registry.is_empty() {
        run_registries(session, selections, projection.selections(), &[], budget)
    } else {
        run_registries(
            session,
            selections,
            projection.selections(),
            std::slice::from_ref(registry),
            budget,
        )
    }
}

/// Replay a canonical external decision log through one exact selected Psi
/// registry. The byte boundary is intentional: strict schema decoding is part
/// of accepting external policy input.
pub fn replay_psi_registry(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    encoded_external_decisions: &[u8],
) -> Result<OptimizationRun, OptimizationRunError> {
    let projection = selections.project_psi();
    let external_decisions =
        ExternalDecisionLog::decode(encoded_external_decisions).map_err(|error| {
            OptimizationRunError::ExternalDecisionReplay(ExternalDecisionReplayError::Schema(error))
        })?;
    let expected =
        built_in_psi_registry(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    if expected.identity() != registry.identity()
        || expected.contracts().collect::<Vec<_>>() != registry.contracts().collect::<Vec<_>>()
    {
        return Err(OptimizationRunError::SelectionRegistryMismatch);
    }
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    if registry.is_empty() {
        run_registries_with_external_decisions(
            session,
            selections,
            projection.selections(),
            &[],
            budget,
            external_decisions,
        )
    } else {
        run_registries_with_external_decisions(
            session,
            selections,
            projection.selections(),
            std::slice::from_ref(registry),
            budget,
            external_decisions,
        )
    }
}

/// Execute every implemented named optimization as its own canonical pass
/// group and publish one chained run over the exact selected suite.
pub fn run_psi_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let projection = selections.project_psi();
    run_psi_pipeline_for_projection(verified, selections, &projection, budget_per_pass)
}

/// Execute the Psi schedule from the coordinator's one bound projection.
/// The complete selection is retained for cross-phase custody but is never
/// rescanned to rediscover this phase's schedule.
pub fn run_psi_pipeline_for_projection(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    projection: &PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    if projection.complete_selection() != selections.identity() {
        return Err(OptimizationRunError::SelectionRegistryMismatch);
    }
    let registries = built_in_psi_registries_for_selections(projection.selections())
        .map_err(OptimizationRunError::RegistryConstruction)?;
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    run_registries(
        session,
        selections,
        projection.selections(),
        &registries,
        budget_per_pass,
    )
}

/// Replay a canonical external decision log through the ordinary selected Psi
/// pipeline. Candidate construction and validation are identical to the
/// model-free run; the log supplies only the action after validation.
pub fn replay_psi_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    encoded_external_decisions: &[u8],
) -> Result<OptimizationRun, OptimizationRunError> {
    let projection = selections.project_psi();
    let external_decisions =
        ExternalDecisionLog::decode(encoded_external_decisions).map_err(|error| {
            OptimizationRunError::ExternalDecisionReplay(ExternalDecisionReplayError::Schema(error))
        })?;
    let registries =
        built_in_psi_registries(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    run_registries_with_external_decisions(
        session,
        selections,
        projection.selections(),
        &registries,
        budget_per_pass,
        external_decisions,
    )
}
