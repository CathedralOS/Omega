use crate::{CallArgument, CallRequirement, DeclarationId, EdgeId};

/// The closed set of non-crashing external terminal outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCompletionKind {
    /// The canonical `ProcessExit::exit_process` requirement ends the bound process domain.
    ProcessExit,
}

/// A terminal invocation of an exact canonical requirement. It has no normal
/// result, successor, normal-return guarantee or cleanup: the bound domain is
/// abandoned, including outstanding obligations, without disposition receipts.
#[derive(Debug, Eq, PartialEq)]
pub struct ExternalExit {
    pub id: EdgeId,
    pub kind: ExternalCompletionKind,
    /// The canonical boundary trait, including its package.
    pub owner: DeclarationId,
    /// The exact canonical requirement the terminal event belongs to.
    pub requirement: DeclarationId,
    /// Evaluated operands in authored formal order.
    pub arguments: Vec<CallArgument>,
    /// One caller obligation per published requirement clause, at this edge.
    pub requirements: Vec<CallRequirement>,
}
