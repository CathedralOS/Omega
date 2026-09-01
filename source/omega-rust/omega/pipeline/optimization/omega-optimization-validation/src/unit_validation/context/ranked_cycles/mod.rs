//! Optimizer module role: executable entrance. Ranked-cycle topology, identity, and immutable-body coordination.

use super::super::*;

mod components;
mod countdown_ranking;
mod freeze;
mod graph;
mod model;
mod replay;
mod topology;

pub use countdown_ranking::validate_psi_ranking_certificate_snapshot;
pub use model::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
};

pub(super) struct RankedCycleAdmission {
    pub(super) policy: function_structure::ControlCyclePolicy,
    pub(super) snapshot: OptimizerCycleComponentSnapshot,
    pub(super) rankings: OptimizerRankingCertificateSnapshot,
}

pub(super) fn validate_exact_ranked_cycles(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<RankedCycleAdmission, OptimizationUnitValidationError> {
    let snapshot = replay::rederive_exact_components(input.context().module(), unit)?;
    let rankings =
        countdown_ranking::rederive_exact_certificates(input.context().module(), unit, &snapshot)?;
    freeze::validate_frozen_component_blocks(
        input,
        unit,
        &snapshot.components,
        &rankings.certificates,
    )?;
    let mut policy = function_structure::ControlCyclePolicy::default();
    for component in &snapshot.components {
        policy.admit(component.id.machine);
    }
    Ok(RankedCycleAdmission {
        policy,
        snapshot,
        rankings,
    })
}
