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
pub use model::{ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates};

use optimization_unit::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
};

pub(super) struct RankedCycleAdmission {
    pub(super) machines: Vec<MachineId>,
    pub(super) snapshot: OptimizerCycleComponentSnapshot,
    pub(super) rankings: OptimizerRankingCertificateSnapshot,
}

pub(super) fn validate_exact_ranked_cycles(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
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
    let machines = snapshot
        .components
        .iter()
        .map(|component| component.id.machine)
        .collect();
    Ok(RankedCycleAdmission {
        machines,
        snapshot,
        rankings,
    })
}
