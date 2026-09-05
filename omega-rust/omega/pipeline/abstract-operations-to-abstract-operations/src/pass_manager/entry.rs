use optimization_core::ExternalDecisionLog;
use optimization_core::{
    OptimizationSelections, OptimizationWorkBudget, PsiOptimizationSelectionProjection,
};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

use crate::{
    OrderedRuleRegistry, built_in_psi_registries, built_in_psi_registries_for_selections,
    built_in_psi_registry,
};

use super::{
    ExternalDecisionReplayError, OptimizationRun, OptimizationRunError,
    VerifiedPsiOptimizationSession,
    execution::{run_registries, run_registries_with_external_decisions},
};

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
