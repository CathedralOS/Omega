//! TPR/EFX: normalized termination plans keyed by exact machine identity.
//!
//! Published interface, checked summary, and private ranking witness remain
//! one plan, independent from the aggregate machine-contract carrier.

use language_semantics::MachineTerminationPlan;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

mod arrival_correspondence;
pub use arrival_correspondence::{
    CheckedMachineRankCorrespondence, CheckedRankArrivalEdge, CheckedRankArrivalPlan,
    CheckedRankBodyJudgment, CheckedRankCoordinate, CheckedRankCoordinateBinding,
    CheckedRankCoordinateHandle, CheckedRankCoordinateTransport, CheckedRankLocalCapture,
    CheckedRankPremiseRegime, CheckedRankReceivingCoordinate, CheckedRankValueDerivation,
    CheckedRuntimeNaturalState, CheckedRuntimeRankComponent, CheckedRuntimeRankEdge,
    CheckedRuntimeRankMember, CheckedSourceNaturalRankComponent, CheckedSourceNaturalRankEdge,
    CheckedSourceNaturalRankMember, CheckedStateRankCorrespondence,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminationFacts {
    /// Coherent source-admitted arrival coordinates. Observation production
    /// consumes this choice rather than rediscovering a carrier by type.
    pub rank_arrivals: CheckedRankArrivalPlan,
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineTerminationFact>,
    /// Accepted proof-only recursive components. The common relation/type are
    /// component-owned and every exact internal call site retains an
    /// independent strict-subterm witness.
    pub proof_recursive_components: Vec<CheckedProofRecursiveComponent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedProofRankingRelation {
    StructuralSubterm,
    StructuralSubtermCycles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedProofRecursiveCallSite {
    Statement {
        state: SymbolHandle,
        statement_index: usize,
    },
    Expression {
        state: SymbolHandle,
        statement_index: usize,
        expression_ordinal: usize,
    },
    Transition {
        state: SymbolHandle,
        statement_index: usize,
        lane: CheckedProofRecursiveTransitionLane,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedProofRecursiveTransitionLane {
    Target,
    Continuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProofRecursiveMember {
    pub machine: SymbolHandle,
    pub rank_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProofRecursiveEdge {
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub site: CheckedProofRecursiveCallSite,
    pub caller_rank_parameter: SymbolHandle,
    pub callee_rank_parameter: SymbolHandle,
    /// Empty only for a preserving edge under StructuralSubtermCycles.
    pub strict_member_path: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProofRecursiveComponent {
    pub members: Vec<CheckedProofRecursiveMember>,
    pub ranking_relation: CheckedProofRankingRelation,
    pub rank_type_identity: TypeIdentityId,
    pub edges: Vec<CheckedProofRecursiveEdge>,
}

impl TerminationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineTerminationPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| &fact.plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineTerminationFact {
    pub machine: SymbolHandle,
    pub plan: MachineTerminationPlan,
}
