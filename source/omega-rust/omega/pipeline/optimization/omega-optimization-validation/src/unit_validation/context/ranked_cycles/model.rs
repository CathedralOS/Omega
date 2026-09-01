//! Optimizer module role: carrier leaf. Canonical component identities and optimizer-analysis custody.

use super::*;

/// One canonical executable control edge belonging to or crossing an SCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
}

/// Semantic identity of one finite cyclic component.
///
/// The owning machine and complete canonical internal-edge roster are the
/// identity. Members and boundary edges are derived topology carried beside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentId {
    pub machine: MachineId,
    pub internal_edges: Vec<CycleComponentEdge>,
}

/// Current optimizer topology for one independently authenticated component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponent {
    pub id: CycleComponentId,
    pub members: Vec<BlockId>,
    pub entries: Vec<CycleComponentEdge>,
    pub exits: Vec<CycleComponentEdge>,
}

/// Replayable, non-authoritative component snapshot.
///
/// Callers may persist or mutate this data. Only
/// [`ValidatedOptimizerCycleComponents`] confers optimizer-analysis authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponentSnapshot {
    pub terminal_psi: psi_terminal::TerminalPsiIdentity,
    pub components: Vec<OptimizerCycleComponent>,
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
    pub(crate) const fn new(
        snapshot: OptimizerCycleComponentSnapshot,
        rankings: ValidatedOptimizerRankingCertificates,
    ) -> Self {
        Self { snapshot, rankings }
    }

    pub const fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
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

/// The closed unsigned-countdown ranking rule currently understood by the
/// optimizer. Every coordinate is retained so later loop analyses need not
/// reinterpret the Terminal ranked-SCC row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerUnsignedCountdownRankingCertificate {
    pub component: CycleComponentId,
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    pub guard: OptimizerUnsignedPositiveGuard,
    pub descent: OptimizerUnsignedMinusOneDescent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedPositiveGuard {
    pub block: BlockId,
    pub edge: EdgeId,
    pub condition: ValueId,
    pub parameter: ValueId,
    pub zero: ValueId,
    pub zero_operation: OperationId,
    pub comparison_operation: OperationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedMinusOneDescent {
    pub backedge: CycleComponentEdge,
    pub argument_index: u32,
    pub argument: ValueId,
    pub source_parameter: ValueId,
    pub target_parameter: ValueId,
    pub one: ValueId,
    pub one_operation: OperationId,
    pub subtract_operation: OperationId,
    pub subtract_obligation: psi_core::ObligationId,
}

/// Replayable ranking-certificate data without analysis authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerRankingCertificateSnapshot {
    pub terminal_psi: psi_terminal::TerminalPsiIdentity,
    pub certificates: Vec<OptimizerUnsignedCountdownRankingCertificate>,
}

/// Opaque optimizer-analysis custody for independently reconstructed ranking
/// evidence. This does not authorize execution or cyclic rewriting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizerRankingCertificates {
    snapshot: OptimizerRankingCertificateSnapshot,
}

impl ValidatedOptimizerRankingCertificates {
    pub(crate) const fn new(snapshot: OptimizerRankingCertificateSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn certificates(&self) -> &[OptimizerUnsignedCountdownRankingCertificate] {
        &self.snapshot.certificates
    }

    pub const fn snapshot(&self) -> &OptimizerRankingCertificateSnapshot {
        &self.snapshot
    }
}
