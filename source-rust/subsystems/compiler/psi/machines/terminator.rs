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
    /// Observe an actual empty value domain. This is neither a return nor
    /// a crash, and transfers no custody. Admission reconstructs emptiness
    /// at the exact live source path, including a final case namespace.
    EliminateEmpty {
        edge: EdgeId,
        source: PlaceId,
        path: Vec<crate::StructuralPathSegment>,
    },
    InvokeNoNormal {
        edge: EdgeId,
        static_reach_binding: Option<u32>,
        required_contract: Option<semantic_vocabulary::ContractId>,
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<crate::ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<crate::CrashRouteBucket>,
    },
    ExternalCompletion {
        edge: EdgeId,
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        requirement_obligations: Vec<ObligationId>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    StructuralCase {
        source: PlaceId,
        path: Vec<crate::StructuralPathSegment>,
        cases: Vec<StructuralCaseSuccessorEdge>,
    },
    Return {
        edge: EdgeId,
        value: ValueId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnUnit {
        edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnStructural {
        edge: EdgeId,
        source: PlaceId,
        returned_claims: Vec<StructuralResultClaimBinding>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Crash {
        edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

pub use semantic_vocabulary::CrashCause;

impl Terminator {
    pub fn storage_uses(&self) -> Vec<crate::OperationProjection<'_>> {
        self.storage_uses_with_arguments(|_| true)
    }

    /// Branch and tail-call arguments may transport a pointer without
    /// materializing its referent. Cleanup and inspected values still count.
    pub fn storage_uses_with_arguments(
        &self,
        include: impl Fn(&StructuralArgument) -> bool,
    ) -> Vec<crate::OperationProjection<'_>> {
        let mut projections = Vec::new();
        fn argument(argument: &StructuralArgument) -> crate::OperationProjection<'_> {
            crate::OperationProjection {
                field: None,
                root: argument.place,
                path: &argument.path,
            }
        }
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
                projections.extend(
                    structural_arguments
                        .iter()
                        .filter(|arg| include(arg))
                        .map(argument),
                );
            }
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    projections.extend(
                        edge.structural_arguments
                            .iter()
                            .filter(|arg| include(arg))
                            .map(argument),
                    );
                }
            }
            Self::StructuralCase { source, path, .. }
            | Self::EliminateEmpty { source, path, .. } => {
                projections.push(crate::OperationProjection {
                    field: None,
                    root: *source,
                    path,
                });
            }
            Self::ReturnStructural { source, .. } => {
                projections.push(crate::OperationProjection {
                    field: None,
                    root: *source,
                    path: &[],
                });
            }
            Self::Return { .. } | Self::ReturnUnit { .. } | Self::Crash { .. } => {}
        }
        for (_, actions) in self.cleanup_rosters() {
            for action in actions {
                projections.push(crate::OperationProjection {
                    field: None,
                    root: action.place(),
                    path: action.path(),
                });
            }
        }
        projections
    }

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
            Self::StructuralCase { source, cases, .. } => {
                *source = map(*source);
                for case in cases {
                    for argument in &mut case.structural_arguments {
                        argument.place = map(argument.place);
                    }
                }
            }
            Self::ReturnStructural { source, .. } | Self::EliminateEmpty { source, .. } => {
                *source = map(*source);
            }
            Self::Return { .. } | Self::ReturnUnit { .. } | Self::Crash { .. } => {}
        }
    }

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
            Self::Crash { .. }
            | Self::EliminateEmpty { .. }
            | Self::ExternalCompletion { .. }
            | Self::InvokeNoNormal { .. } => (None, None, &[]),
        };
        first.into_iter().chain(second).chain(
            cases
                .iter()
                .map(|case| (case.edge, case.cleanup_actions.as_slice())),
        )
    }

    pub fn cleanup_rosters_mut(
        &mut self,
    ) -> impl Iterator<Item = (EdgeId, &mut Vec<TerminalAffineCleanupAction>)> {
        let (first, second, cases): (_, _, &mut [StructuralCaseSuccessorEdge]) = match self {
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
            } => (Some((*edge, cleanup_actions)), None, &mut []),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (
                Some((when_true.edge, &mut when_true.cleanup_actions)),
                Some((when_false.edge, &mut when_false.cleanup_actions)),
                &mut [],
            ),
            Self::StructuralCase { cases, .. } => (None, None, cases),
            Self::Crash { .. }
            | Self::EliminateEmpty { .. }
            | Self::ExternalCompletion { .. }
            | Self::InvokeNoNormal { .. } => (None, None, &mut []),
        };
        first.into_iter().chain(second).chain(
            cases
                .iter_mut()
                .map(|case| (case.edge, &mut case.cleanup_actions)),
        )
    }

    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::InvokeNoNormal { edge, .. }
            | Self::EliminateEmpty { edge, .. }
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
            | Self::EliminateEmpty { edge, .. }
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
            Self::StructuralCase { path, .. } | Self::EliminateEmpty { path, .. } => {
                for segment in path {
                    if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        *index = map(*index);
                    }
                }
            }
            Self::ReturnUnit { .. } | Self::ReturnStructural { .. } | Self::Crash { .. } => {}
        }
    }
    /// Every successor edge in edge order. Jump and conditional edges bind
    /// their own operands; a dispatch edge binds from the selected payload.
    fn successors(&self) -> impl Iterator<Item = Successor<'_>> {
        let (jump, branches, cases) = match self {
            Self::Jump {
                edge,
                target,
                arguments,
                structural_arguments,
                ..
            } => {
                let jump = Successor {
                    edge: *edge,
                    target: *target,
                    arguments,
                    structural_arguments,
                    dispatch: false,
                };
                (Some(jump), None, &[][..])
            }
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (None, Some([when_true, when_false]), &[][..]),
            Self::StructuralCase { cases, .. } => (None, None, cases.as_slice()),
            Self::EliminateEmpty { .. }
            | Self::Return { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::Crash { .. }
            | Self::ExternalCompletion { .. }
            | Self::InvokeNoNormal { .. } => (None, None, &[][..]),
        };
        let branches = branches.into_iter().flatten().map(|branch| Successor {
            edge: branch.edge,
            target: branch.target,
            arguments: &branch.arguments,
            structural_arguments: &branch.structural_arguments,
            dispatch: false,
        });
        let cases = cases.iter().map(|case| Successor {
            edge: case.edge,
            target: case.target,
            arguments: &[],
            structural_arguments: &case.structural_arguments,
            dispatch: true,
        });
        jump.into_iter().chain(branches).chain(cases)
    }

    pub fn successor_block_arguments(&self) -> Vec<&[StructuralArgument]> {
        self.successors()
            .map(|successor| successor.structural_arguments)
            .collect()
    }

    /// Jump and conditional edges that bind scalar or structural values at
    /// their destination. Dispatch edges bind from the selected payload.
    pub fn successors_with_runtime_arguments(&self) -> impl Iterator<Item = (EdgeId, BlockId)> {
        self.successors()
            .filter(|successor| {
                !successor.dispatch
                    && (!successor.arguments.is_empty()
                        || !successor.structural_arguments.is_empty())
            })
            .map(|successor| (successor.edge, successor.target))
    }

    pub fn successor_edges(&self) -> Vec<(EdgeId, BlockId)> {
        self.successors()
            .map(|successor| (successor.edge, successor.target))
            .collect()
    }

    pub fn successor_targets(&self) -> Vec<BlockId> {
        self.successor_edges()
            .into_iter()
            .map(|(_, target)| target)
            .collect()
    }
}

struct Successor<'a> {
    edge: EdgeId,
    target: BlockId,
    arguments: &'a [ValueId],
    structural_arguments: &'a [StructuralArgument],
    dispatch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    pub erased_arguments: Vec<ScalarTerm>,
    pub erased_proof_arguments: Vec<ProofTerm>,
    pub structural_arguments: Vec<StructuralArgument>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralCaseSuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub payload_fields: Vec<StructuralFieldId>,
    pub structural_arguments: Vec<StructuralArgument>,
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
