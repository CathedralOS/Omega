use crate::{
    CrashPredicateTerm, StructuralArgument, StructuralResultClaimBinding,
    TerminalAffineCleanupAction,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, ExternalCompletionKind, MachineId, ObligationId,
    PlaceId, ProofTerm, ScalarTerm, StructuralCaseId, StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Invoke an exact module machine which cannot complete normally. Ordinary
    /// entry obligations, argument custody and callee crash routes still apply.
    /// There is no receiving result, normal cleanup, successor or abandonment
    /// receipt. The module's checked CFG must establish absence of normal exit;
    /// this marker is neither a crash event nor authority to elide the caller frame.
    InvokeNoNormal {
        edge: EdgeId,
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<crate::ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<crate::CrashRouteBucket>,
    },
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
    /// edge, before that edge's cleanup. Inspection itself consumes no custody.
    StructuralCase {
        source: PlaceId,
        /// Select the inspected sum within the original live storage. Payload
        /// capture precedes edge cleanup; a projection never consumes its root.
        path: Vec<crate::StructuralPathSegment>,
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
        /// Claim identities and exact typed paths beneath `source`, ordered by
        /// identity. Case segments guard payload custody; inactive paths carry
        /// no live permission. The paths make the return-to-caller join explicit
        /// even when the selected sum has not been dispatched in this machine.
        returned_claims: Vec<StructuralResultClaimBinding>,
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
    /// Rewrite runtime structural root operands, excluding cleanup and proof
    /// identities. This inventory describes uses, not authority to consume them.
    pub fn map_structural_uses(&mut self, map: &mut impl FnMut(PlaceId) -> PlaceId) {
        match self {
            Self::Jump {
                structural_arguments,
                ..
            }
            | Self::InvokeNoNormal {
                structural_arguments,
                ..
            }
            | Self::ExternalCompletion {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    argument.place = map(argument.place);
                }
            }
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    for argument in &mut edge.structural_arguments {
                        argument.place = map(argument.place);
                    }
                }
            }
            Self::StructuralCase { source, .. } | Self::ReturnStructural { source, .. } => {
                *source = map(*source);
            }
            Self::Return { .. } | Self::ReturnUnit { .. } | Self::Crash { .. } => {}
        }
    }

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
            Self::Crash { .. } | Self::ExternalCompletion { .. } | Self::InvokeNoNormal { .. } => {
                (None, None, &[])
            }
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
            Self::InvokeNoNormal { edge, .. }
            | Self::Jump { edge, .. }
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
            Self::InvokeNoNormal { edge, .. }
            | Self::Jump { edge, .. }
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
            Self::Jump { arguments, .. }
            | Self::ExternalCompletion { arguments, .. }
            | Self::InvokeNoNormal { arguments, .. } => {
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
            Self::StructuralCase { path, .. } => {
                for segment in path {
                    if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        *index = map(*index);
                    }
                }
            }
            Self::ReturnUnit { .. } | Self::ReturnStructural { .. } | Self::Crash { .. } => {}
        }
    }
    /// Structural-argument rosters this terminator binds into successor
    /// blocks: the jump's roster or each conditional successor's roster, in
    /// edge order. External completions feed a boundary rather than a block,
    /// and case successors bind payloads positionally — neither contributes a
    /// roster.
    pub fn successor_block_arguments(&self) -> Vec<&[StructuralArgument]> {
        match self {
            Self::Jump {
                structural_arguments,
                ..
            } => vec![structural_arguments.as_slice()],
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                when_true.structural_arguments.as_slice(),
                when_false.structural_arguments.as_slice(),
            ],
            _ => Vec::new(),
        }
    }

    /// Edge-and-target pairs of every successor branch.
    pub fn successor_edges(&self) -> Vec<(EdgeId, BlockId)> {
        match self {
            Self::Jump { edge, target, .. } => vec![(*edge, *target)],
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ],
            Self::StructuralCase { cases, .. } => cases
                .iter()
                .map(|successor| (successor.edge, successor.target))
                .collect(),
            Self::Return { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::Crash { .. }
            | Self::ExternalCompletion { .. }
            | Self::InvokeNoNormal { .. } => Vec::new(),
        }
    }

    /// Target blocks of every successor branch.
    pub fn successor_targets(&self) -> Vec<BlockId> {
        self.successor_edges()
            .into_iter()
            .map(|(_, target)| target)
            .collect()
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

#[macro_export]
macro_rules! terminator_return_pair_pattern {
    ($($fields:tt)*) => {
        $crate::Terminator::Return { $($fields)* }
            | $crate::Terminator::ReturnUnit { $($fields)* }
    };
}

#[macro_export]
macro_rules! return_or_return_structural_pattern {
    ($($fields:tt)*) => {
        $crate::Terminator::Return { $($fields)* }
            | $crate::Terminator::ReturnStructural { $($fields)* }
    };
}
