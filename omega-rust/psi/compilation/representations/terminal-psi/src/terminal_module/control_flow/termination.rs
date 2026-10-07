use crate::{CrashPredicateTerm, StructuralArgument, TerminalAffineCleanupAction};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, ExternalCompletionKind, ObligationId, PlaceId,
    ProofTerm, ScalarTerm, StructuralCaseId, StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Invoke the exact boundary's closed external completion without a normal
    /// result, successor, cleanup, or disposition receipt. This demands actual
    /// delegated authority bound to the instance's containing process domain;
    /// the terminator itself grants none. Admission must establish that binding
    /// for this exact slot before execution.
    ///
    /// `frontier_lower_bound` retains only reconstructible machine-local claims,
    /// not a complete process-wide obligation census or a discharge claim.
    ExternalCompletion {
        edge: EdgeId,
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        requirement_obligations: Vec<ObligationId>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    /// Simultaneously bind target block parameters from the listed values.
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
        /// Proof-only scalar terms for the target block's erased formals, in
        /// the predecessor's namespace and the roster's authored order. They
        /// carry no runtime operand.
        erased_arguments: Vec<ScalarTerm>,
        /// Proof-only actuals for the target block's `erased_proof_formals`
        /// roster, in roster order.
        erased_proof_arguments: Vec<ProofTerm>,
        /// Positional bindings for the target block's structural parameters.
        structural_arguments: Vec<StructuralArgument>,
        /// Ordered disposal after outgoing operands are captured and before
        /// destination bindings are installed. Only this selected edge runs it.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
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
        /// Exact semantic action order, shared with scalar and structural
        /// returns. Result absence does not change owner disposal semantics.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Transfer one structural value and its complete live claim set to the
    /// machine result. Fuel is charged before any transfer or cleanup commits.
    ReturnStructural {
        edge: EdgeId,
        source: PlaceId,
        /// Strictly ordered exact live claims transferred with `source`.
        returned_claims: Vec<ClaimId>,
        /// Exact ordered actions committed after result and claim materialization.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
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
    /// Enumerate edge-owned cleanup dependencies without selecting or executing
    /// a branch. Each roster keeps its authored action order and edge identity.
    pub fn cleanup_rosters(
        &self,
    ) -> impl Iterator<Item = (EdgeId, &[TerminalAffineCleanupAction])> {
        let (first, second, cases): (_, _, &[StructuralCaseSuccessorEdge]) = match self {
            Self::Jump {
                edge,
                cleanup_actions,
                ..
            }
            | Self::Return {
                edge,
                cleanup_actions,
                ..
            }
            | Self::ReturnUnit {
                edge,
                cleanup_actions,
            }
            | Self::ReturnStructural {
                edge,
                cleanup_actions,
                ..
            } => (Some((*edge, cleanup_actions.as_slice())), None, &[]),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (
                Some((when_true.edge, when_true.cleanup_actions.as_slice())),
                Some((when_false.edge, when_false.cleanup_actions.as_slice())),
                &[],
            ),
            Self::StructuralCase { cases, .. } => (None, None, cases),
            Self::Crash { .. } | Self::ExternalCompletion { .. } => (None, None, &[]),
        };
        first.into_iter().chain(second).chain(
            cases
                .iter()
                .map(|case| (case.edge, case.cleanup_actions.as_slice())),
        )
    }

    /// The sole edge of an unconditional terminator.
    ///
    /// Conditional consumers must use [`Self::edges`] or inspect the selected
    /// successor instead of silently treating one arm as the terminator edge.
    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. }
            | Self::ExternalCompletion { edge, .. } => *edge,
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
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. }
            | Self::ExternalCompletion { edge, .. } => vec![*edge],
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.edge, when_false.edge],
            Self::StructuralCase { cases, .. } => cases.iter().map(|case| case.edge).collect(),
        };
        edges.into_iter()
    }

    /// Rewrite every scalar `ValueId` use site through `map`.
    ///
    /// Jump, conditional successor and external-completion arguments, the
    /// conditional condition, and the returned value are uses. Structural-case payload bindings are
    /// positional implicit bindings rather than listed uses, and `Crash` proof
    /// sites (`site_guard` predicates, claim frontiers) carry value identities
    /// inside propositions rather than direct uses — consumers replacing a
    /// value must keep machines containing those carriers unchanged or handle
    /// them separately.
    pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Jump { arguments, .. } | Self::ExternalCompletion { arguments, .. } => {
                for argument in arguments {
                    *argument = map(*argument);
                }
            }
            Self::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                *condition = map(*condition);
                for edge in [when_true, when_false] {
                    for argument in &mut edge.arguments {
                        *argument = map(*argument);
                    }
                }
            }
            Self::Return { value, .. } => {
                *value = map(*value);
            }
            Self::StructuralCase { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::Crash { .. } => {}
        }
    }
}

/// One ordered conditional successor and its simultaneous block-parameter
/// scalar and structural bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    /// Proof-only scalar terms for the target block's erased formals, in the
    /// predecessor's namespace and the roster's authored order. They carry no
    /// runtime operand.
    pub erased_arguments: Vec<ScalarTerm>,
    /// Proof-only actuals for the target block's `erased_proof_formals`
    /// roster, in roster order.
    pub erased_proof_arguments: Vec<ProofTerm>,
    /// Positional bindings for the target block's structural parameters.
    pub structural_arguments: Vec<StructuralArgument>,
    /// Ordered actions committed only on this selected edge.
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
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
    /// Ordered actions after payload capture and before destination binding.
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

impl SuccessorEdge {
    /// Roots whose whole value or exact remaining subtrees this edge disposes.
    pub fn affine_discard_roots(&self) -> impl Iterator<Item = PlaceId> + '_ {
        self.cleanup_actions.iter().map(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            TerminalAffineCleanupAction::DiscardResidual(discard) => discard.place,
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => cleanup.place,
        })
    }
}

impl StructuralCaseSuccessorEdge {
    /// Roots whose whole value or exact remaining subtrees this edge disposes.
    pub fn affine_discard_roots(&self) -> impl Iterator<Item = PlaceId> + '_ {
        self.cleanup_actions.iter().map(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            TerminalAffineCleanupAction::DiscardResidual(discard) => discard.place,
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => cleanup.place,
        })
    }
}
