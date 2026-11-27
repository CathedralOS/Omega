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
    /// Retire the home after its active regions have been disposed in order.
    /// This action alone discharges no claims or loans.
    RetireHome(PlaceId),
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
        /// When set, normal completion binds the call result to the target block's
        /// parameter (none for Unit) and runs its cleanup graph before returning.
        /// In that form `cleanup` is empty. With no target, the ordered actions
        /// in `cleanup` run after the call completes and the result returns directly.
        completion: Option<BlockId>,
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
    /// One row per domain this jump establishes at an argument, each with its own origin.
    /// A replacement also names its qualification change at this argument. The rows
    /// cover just the added domains; removed domains are read from the two value types.
    pub domain_completions: Vec<DomainCompletion>,
    /// Each structural input transfers or copies into its corresponding target parameter.
    pub structural_arguments: Vec<crate::ValueOperand>,
    pub cleanup: Vec<CleanupAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseSelection {
    pub source:crate::ProjectedPlace,
    pub case:crate::DeclarationId,
}

/// On this jump, the fresh destination parameter at `argument_index` gains `domain`,
/// which its argument lacks. `domain` is the exact atom of the destination type,
/// with its ordered closed arguments, so `Above<5>` and `Above<6>` are different rows.
/// `origin` is that membership's scalar return origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainCompletion {
    pub argument_index: usize,
    pub domain: crate::DomainAtom,
    pub origin: CompletionOrigin,
}

/// The completion origins of `source-rust/subsystems/compiler/psi/artifact/wire/encoding.md` (scalar return origins).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionOrigin {
    /// A predicate-free, route-free tag. Its carrier’s required memberships must
    /// arrive on this argument or be independently established here.
    Vacuous,
    /// An unrestricted predicate domain. The destination's block invariant retains the
    /// predicate, and every arrival owes it; the row itself is not a proof.
    Proved,
}

/// The named domain's complete `requires` list with `self` bound to this block parameter
/// and each index binder bound to the atom's closed argument in head order.
/// Refers to the owned declaration; no formula, including a called predicate, is re-selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarDomainInvariant {
    pub subject: ValueId,
    pub domain: crate::DomainAtom,
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
    /// One authored cast both removes and adds memberships on this argument.
    /// Every added domain has a completion row on the same edge and ordinal.
    Replace,
}
