//! Sealed authority for independently reconstructed cycle and ranking snapshots.

use super::*;

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
