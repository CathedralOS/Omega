//! Optimizer module role: executable entrance. Verified-cycle topology, identity, and immutable-body coordination.

use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::MachineId;

mod components;
mod countdown_ranking;
mod freeze;
mod graph;
mod ordinary;
mod replay;
mod topology;

pub use countdown_ranking::validate_psi_ranking_certificate_snapshot;

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
    freeze::validate_frozen_component_blocks(input, unit, &snapshot.components)?;
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

/// Opaque authority to use the contained SCC topology for optimizer analysis.
///
/// This grants no Terminal execution, rewrite, interpretation, fixed-fuel,
/// native-lowering, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizerCycleComponents {
    snapshot: OptimizerCycleComponentSnapshot,
    rankings: ValidatedOptimizerRankingCertificates,
}

impl ValidatedOptimizerCycleComponents {
    pub(in crate::validation::context) const fn new(
        snapshot: OptimizerCycleComponentSnapshot,
        rankings: ValidatedOptimizerRankingCertificates,
    ) -> Self {
        Self { snapshot, rankings }
    }

    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.snapshot.terminal_psi
    }

    pub fn components(&self) -> &[OptimizerCycleComponent] {
        &self.snapshot.components
    }

    pub const fn snapshot(&self) -> &OptimizerCycleComponentSnapshot {
        &self.snapshot
    }

    /// Exact well-founded evidence available to optimizer analyses only.
    pub const fn ranking_certificates(&self) -> &ValidatedOptimizerRankingCertificates {
        &self.rankings
    }
}

/// Opaque optimizer-analysis custody for independently reconstructed ranking
/// evidence. This does not authorize execution or cyclic rewriting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizerRankingCertificates {
    snapshot: OptimizerRankingCertificateSnapshot,
}

impl ValidatedOptimizerRankingCertificates {
    pub(in crate::validation::context) const fn new(
        snapshot: OptimizerRankingCertificateSnapshot,
    ) -> Self {
        Self { snapshot }
    }

    pub fn certificates(&self) -> &[OptimizerUnsignedCountdownRankingCertificate] {
        &self.snapshot.certificates
    }

    pub const fn snapshot(&self) -> &OptimizerRankingCertificateSnapshot {
        &self.snapshot
    }
}
