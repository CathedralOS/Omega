//! Rank claims cite actual control transfers, never just a callee or source span.
use crate::{
    BlockId, BlockTerminator, BranchBlock, CallArgument, CallSite, CallTarget, EdgeId, LoweringError, MachineBody, OperationId, OperationKind, OperationResult,
    PsiOperation, ReturnEdge, Site, TypeShape, TypeTable,
};

#[derive(Clone, Copy)]
pub(super) enum Invocation<'a> {
    Ordinary(&'a BranchBlock, &'a PsiOperation),
    Terminal(&'a BranchBlock),
    Cleanup(&'a Site),
}

pub(super) struct RankedTransfer<'a> {
    pub block: BlockId,
    pub edge: EdgeId,
    pub forwarded_call: Option<OperationId>,
    pub arguments: &'a [CallArgument],
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
                    edge: *id,
                    forwarded_call: None,
                    arguments,
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
                // In this form there is no work to resume, parked activation,
                // caller cleanup or borrowed backing to carry across iterations.
                if let BlockTerminator::Return {
                    id,
                    result: ReturnEdge::Scalar(value),
                    cleanup,
                } = &block.terminator
                    && block.operations.end == op.id.get()
                    && op.result == OperationResult::Scalar(*value)
                    && primitive(*value)
                    && cleanup.is_empty()
                    && matches!(callee, CallTarget::Machine(_))
                    && arguments
                        .iter()
                        .all(|arg| matches!(arg, CallArgument::Scalar(v) if primitive(*v)))
                    && !body
                        .suspension_sites
                        .iter()
                        .any(|s| s.call == CallSite::Operation(op.id))
                {
                    return Ok(RankedTransfer {
                        block: block.id,
                        edge: *id,
                        forwarded_call: Some(op.id),
                        arguments,
                        site,
                    });
                }
                site
            }
            Self::Cleanup(site) => site,
        };
        Err(LoweringError::new(
            site,
            "ranked recursion needs a terminal call or a direct cleanup-free scalar tail transfer",
        ))
    }
}
