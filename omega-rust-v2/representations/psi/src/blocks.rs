use crate::{BlockId, CallArgument, EdgeId, PlaceId, ReturnEdge, ValueId};
use std::ops::Range;

#[derive(Debug, Eq, PartialEq)]
pub struct BranchBlock {
    pub id: BlockId,
    pub scalar_parameters: Vec<ValueId>,
    pub structural_parameters: Vec<PlaceId>,
    /// Every incoming edge owes these predicates, even if it carries no completion request.
    pub scalar_domain_invariants: Vec<ScalarDomainInvariant>,
    pub operations: Range<usize>,
    pub terminator: BlockTerminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupAction {
    DiscardRoot(PlaceId),
    DiscardResidual {
        place: crate::ProjectedPlace,
        ty: crate::TypeId,
    },
    Invoke {
        place: crate::ProjectedPlace,
        ty: crate::TypeId,
        target: crate::MachineId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum BlockTerminator {
    Return {
        id: EdgeId,
        result: ReturnEdge,
        /// Cleanup of surviving owned storage in reverse establishment order.
        cleanup: Vec<CleanupAction>,
    },
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    Jump(SuccessorEdge),
    TerminalCall {
        id: EdgeId,
        callee: crate::CallTarget,
        arguments: Vec<CallArgument>,
        requirements: Vec<crate::CallRequirement>,
        cleanup: Vec<CleanupAction>,
    },
    /// A closed external completion. It is neither a normal return nor a crash.
    ExternalExit(crate::ExternalExit),
}

#[derive(Debug, Eq, PartialEq)]
pub struct SuccessorEdge {
    /// A matched arm identifies the original sum and the case this successor establishes.
    pub selected_case: Option<CaseSelection>,
    /// An authored assembly transition requires adjacency with no machine instruction.
    pub requires_adjacency: bool,
    pub id: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    /// The edge supplies the argument value and its target supplies the fresh parameter at this position.
    pub scalar_qualification_changes: Vec<ScalarQualificationChange>,
    /// Requests for predicate establishment, not tag changes or proof verdicts.
    pub predicate_completions: Vec<PredicateCompletion>,
    /// Each structural input transfers or copies into its corresponding target parameter.
    pub structural_arguments: Vec<crate::ValueOperand>,
    pub cleanup: Vec<CleanupAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseSelection {
    pub source:crate::ProjectedPlace,
    pub case:crate::DeclarationId,
}

/// On this jump, check the argument against the fresh destination parameter at the same ordinal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateCompletion {
    pub argument_index: usize,
    pub domain: crate::DeclarationId,
}

/// The named domain's complete `requires` list with `self` bound to this block parameter.
/// Refers to the owned declaration; no formula, including a called predicate, is re-selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarDomainInvariant {
    pub subject: ValueId,
    pub domain: crate::DeclarationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarQualificationChange {
    pub argument_index: usize,
    pub direction: QualificationDirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationDirection {
    Add,
    Erase,
}
