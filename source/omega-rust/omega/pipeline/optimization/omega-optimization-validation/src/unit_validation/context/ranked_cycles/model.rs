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
}

impl ValidatedOptimizerCycleComponents {
    pub(crate) const fn new(snapshot: OptimizerCycleComponentSnapshot) -> Self {
        Self { snapshot }
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
}
