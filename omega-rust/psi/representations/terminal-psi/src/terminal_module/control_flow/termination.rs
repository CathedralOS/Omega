use crate::{
    CrashPredicateTerm, NominalAffineCleanup, StructuralAffineDiscard, TerminalAffineCleanupAction,
};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Simultaneously bind target block parameters from the listed values.
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
        /// Exact no-code affine discards performed after edge fuel and outgoing
        /// scalar materialization, in reverse parameter declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Select exactly one ordered successor from an already-defined Boolean
    /// value. Exhaustiveness and mutual exclusion are structural.
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    /// Inspect one closed structural sum and select its exact active case.
    /// Payload fields become target-block parameters only on the matching
    /// edge, before that edge consumes the affine source place.
    StructuralCase {
        source: PlaceId,
        cases: Vec<StructuralCaseSuccessorEdge>,
    },
    /// Bind a scalar result, then perform the exact ordered affine cleanup
    /// actions before returning to the caller.
    Return {
        edge: EdgeId,
        value: ValueId,
        /// Semantic execution order. Consumers must preserve this list rather
        /// than regrouping actions by cleanup kind.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Finish normally without producing or binding a runtime value.
    ReturnUnit {
        edge: EdgeId,
        /// Exact no-code affine discards performed after outgoing-value
        /// materialization and before control returns to the caller.
        /// Entries are structural places in reverse declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Finish normally after committing an exact projected transfer and then
    /// disposing only the remaining live affine structural places. This is a
    /// distinct pre-release variant so root-only consumers cannot silently
    /// erase path-sensitive cleanup.
    ReturnUnitPartialAffine {
        edge: EdgeId,
        trivial_affine_discards: Vec<PlaceId>,
        residual_affine_discards: Vec<StructuralAffineDiscard>,
    },
    /// Finish normally after executing the exact ordered nominal cleanups for
    /// whole affine structural parameters. Entries are in reverse parameter
    /// declaration order.
    ReturnUnitNominalAffine {
        edge: EdgeId,
        cleanups: Vec<NominalAffineCleanup>,
    },
    /// Transfer one structural value and its complete live claim set to the
    /// machine result. Fuel is charged before any transfer or cleanup commits.
    ReturnStructural {
        edge: EdgeId,
        source: PlaceId,
        /// Strictly ordered exact live claims transferred with `source`.
        returned_claims: Vec<ClaimId>,
        /// Exact no-code affine discards committed after result materialization.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Leave checked execution without cleanup or a successor.
    ///
    /// `site_guard` is the canonical conjunction known on every path into this
    /// site. `frontier_lower_bound` is deliberately not described as the
    /// complete process-wide abandonment set: it is the machine-local claim
    /// frontier the verifier can reconstruct at this site.
    Crash {
        edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

impl Terminator {
    /// The sole edge of an unconditional terminator.
    ///
    /// Conditional consumers must use [`Self::edges`] or inspect the selected
    /// successor instead of silently treating one arm as the terminator edge.
    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => *edge,
            Self::Conditional { .. } | Self::StructuralCase { .. } => {
                panic!("a branching terminator has multiple successor edges")
            }
        }
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        let edges = match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => vec![*edge],
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.edge, when_false.edge],
            Self::StructuralCase { cases, .. } => cases.iter().map(|case| case.edge).collect(),
        };
        edges.into_iter()
    }
}

/// One ordered conditional successor and its simultaneous block-parameter
/// bindings. The bindings are the current scalar edge-action vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    /// Exact no-code affine discards committed only when this successor is
    /// selected, in reverse parameter declaration order.
    pub trivial_affine_discards: Vec<PlaceId>,
}

/// One exhaustive closed-sum successor. `payload_fields` is positional with
/// the target block's scalar parameters and names only relevant scalar fields
/// declared by this exact case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralCaseSuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub payload_fields: Vec<StructuralFieldId>,
    pub trivial_affine_discards: Vec<PlaceId>,
}
