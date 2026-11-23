//! Rank claims cite actual control transfers, never just a callee or source span.
use crate::{
    BlockId, BlockTerminator, BranchBlock, CallArgument, CallSite, CallTarget, CleanupAction, CleanupList, CleanupSite, EdgeId, LoweringError, MachineBody,
    OperationKind, OperationResult, PsiOperation, ReturnEdge, Site, StructuralStoreDisposition, TypeId, TypeShape, TypeTable,
};

#[derive(Clone, Copy)]
pub(super) enum Invocation<'a> {
    Ordinary(&'a BranchBlock, &'a PsiOperation),
    Terminal(&'a BranchBlock),
    /// A selected nominal cleanup in this block, at this list and ordinal.
    Cleanup(&'a BranchBlock, CleanupSite, &'a Site),
}

/// The actual arguments a target's rank is substituted over.
#[derive(Clone, Copy)]
pub(super) enum Actuals<'a> {
    Call(&'a [CallArgument]),
    /// The cleaned place of this exact type, moved into the target's one formal.
    Cleanup {
        ty: TypeId,
    },
}

pub(super) struct RankedTransfer<'a> {
    pub block: BlockId,
    pub call: CallSite,
    pub forwarding_return: Option<EdgeId>,
    pub actuals: Actuals<'a>,
    pub site: &'a Site,
}

impl<'a> Invocation<'a> {
    pub(super) fn transfer(
        self,
        body: &'a MachineBody,
        types: &TypeTable,
    ) -> Result<RankedTransfer<'a>, LoweringError> {
        let site = match self {
            Self::Terminal(block) => {
                let BlockTerminator::TerminalCall { id, arguments, .. } = &block.terminator else {
                    unreachable!("collected a terminal call")
                };
                return Ok(RankedTransfer {
                    block: block.id,
                    call: CallSite::Edge(*id),
                    forwarding_return: None,
                    actuals: Actuals::Call(arguments),
                    site: super::edge_site(body, id.get()),
                });
            }
            Self::Ordinary(block, op) => {
                let site = &body.diagnostics.operations[op.id.index()];
                let OperationKind::Call { callee, arguments, .. } = &op.kind else {
                    unreachable!("collected a call operation")
                };
                let primitive = |v: crate::ValueId| {
                    body.values
                        .get(v.index())
                        .and_then(|v| types.get(v.ty))
                        .is_some_and(|t| matches!(t, TypeShape::PrimitiveScalar(_)))
                };
                if matches!(op.result, OperationResult::Scalar(value) if primitive(value))
                    && matches!(callee, CallTarget::Machine(_))
                    && arguments
                        .iter()
                        .all(|arg| matches!(arg, CallArgument::Scalar(v) if primitive(*v)))
                    && !body
                        .suspension_sites
                        .iter()
                        .any(|s| s.call == CallSite::Operation(op.id))
                {
                    let forwarding_return = match &block.terminator {
                        BlockTerminator::Return { id, result: ReturnEdge::Scalar(value), cleanup }
                            if block.operations.end == op.id.get() && op.result == OperationResult::Scalar(*value) && cleanup.is_empty() => Some(*id),
                        _ => None,
                    };
                    return Ok(RankedTransfer {
                        block: block.id,
                        call: CallSite::Operation(op.id),
                        forwarding_return,
                        actuals: Actuals::Call(arguments),
                        site,
                    });
                }
                site
            }
            Self::Cleanup(block, at, site) => {
                // Read the action back from the published body, so the claim
                // names exactly the invocation that runs there.
                let list = match at.list {
                    CleanupList::Edge(id) => edge_cleanup(&block.terminator, id),
                    CleanupList::Store(id) => body.operations[block.operations.clone()]
                        .iter()
                        .find(|op| op.id == id)
                        .and_then(|op| match &op.kind {
                            OperationKind::StructuralStore {
                                disposition: StructuralStoreDisposition::Dispose(actions),
                                ..
                            } => Some(actions.as_slice()),
                            _ => None,
                        }),
                };
                let Some(&CleanupAction::Invoke { ty, .. }) = list.and_then(|actions| actions.get(at.action)) else {
                    return Err(LoweringError::new(site, "ranked cleanup does not name an invocation in its block"));
                };
                return Ok(RankedTransfer {
                    block: block.id,
                    call: CallSite::Cleanup(at),
                    forwarding_return: None,
                    actuals: Actuals::Cleanup { ty },
                    site,
                });
            }
        };
        Err(LoweringError::new(
            site,
            "ranked recursion needs a terminal call or a non-suspending direct scalar call",
        ))
    }
}

/// The cleanup-action list of the edge `id` leaving this terminator.
fn edge_cleanup(
    terminator: &BlockTerminator,
    id: EdgeId,
) -> Option<&[CleanupAction]> {
    match terminator {
        BlockTerminator::Return { id: edge, cleanup, .. } | BlockTerminator::TerminalCall { id: edge, cleanup, .. } if *edge == id => Some(cleanup),
        BlockTerminator::Jump(edge) if edge.id == id => Some(&edge.cleanup),
        BlockTerminator::Conditional { when_true, when_false, .. } => [when_true, when_false]
            .into_iter()
            .find(|edge| edge.id == id)
            .map(|edge| edge.cleanup.as_slice()),
        _ => None,
    }
}
