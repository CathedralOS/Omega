//! The coherent coordinate witness selected by source ranking admission.
//!
//! Discovery can retain several same-typed copies. Source checking chooses
//! one complete state-coordinate assignment only after that assignment proves
//! every applicable arrival, formation, endpoint and decrease obligation. The
//! table stores that accepted choice; observation production reads it without
//! repeating discovery or choosing a different sibling. Its paths retain
//! declaration identity, not native storage or permission to read it.

use arena::{Arena, Handle, HandleSpan};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;
use typed_trees::statement::TransitionTargetHandle;
use typed_trees::types::TypeReferenceHandle;

pub type CheckedRankCoordinateHandle = Handle<CheckedRankCoordinate>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedRankValueDerivation {
    #[default]
    CapturedCoordinate,
    ComputedDependency,
}

/// One exact formal-relative coordinate. An empty path is the formal itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankCoordinate {
    pub parameter: SymbolHandle,
    pub path: HandleSpan<PlaceSegment>,
}

/// Binding-time provenance for a stable state-local captured value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankLocalCapture {
    pub symbol: SymbolHandle,
    pub statement_ordinal: u32,
    pub initializer: ExpressionHandle,
    pub occurrence: ExpressionHandle,
}

/// One exact arrival's actual completion. A computation dependency is not an
/// equality premise; source admission still substitutes and judges the actual.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankCoordinateTransport {
    pub source_state: SymbolHandle,
    /// Original parent coordinate in the authenticated transport record.
    pub source: CheckedRankCoordinateHandle,
    /// The selected reading extends the candidate's transported parent by this
    /// exact suffix. Completion remains the original source occurrence; this
    /// does not manufacture an expression or turn a dependency into equality.
    pub selected_suffix: HandleSpan<PlaceSegment>,
    pub statement_ordinal: u32,
    pub target: TransitionTargetHandle,
    pub actual: ExpressionHandle,
    pub completion: ExpressionHandle,
    pub derivation: CheckedRankValueDerivation,
    pub captures: HandleSpan<CheckedRankLocalCapture>,
}

/// A receiving completion locates an independently formed fresh value. It
/// has no entry-value ancestry and cannot export a source equality. The
/// selected binding and complete arrival judgments supply the rank relation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankReceivingCoordinate {
    pub destination: CheckedRankCoordinateHandle,
    pub type_reference: TypeReferenceHandle,
    pub source_state: SymbolHandle,
    pub statement_ordinal: u32,
    pub target: TransitionTargetHandle,
    pub actual: ExpressionHandle,
    pub completion: ExpressionHandle,
}

/// A selected reading. Its entry coordinate denotes the ranking template's
/// subject; its destination denotes this state's own arrival value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankCoordinateBinding {
    pub entry: CheckedRankCoordinateHandle,
    pub destination: CheckedRankCoordinateHandle,
    pub transports: HandleSpan<CheckedRankCoordinateTransport>,
    pub receiving: HandleSpan<CheckedRankReceivingCoordinate>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStateRankCorrespondence {
    pub state: SymbolHandle,
    pub bindings: HandleSpan<CheckedRankCoordinateBinding>,
}

/// The complete proof attempt uses one premise regime on every edge. Initial
/// entry facts remain an edge-local exception, never a renewed arrival fact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedRankPremiseRegime {
    #[default]
    RankInvariant,
    EntryInvariant,
}

/// The arithmetic result for one exact complete local arrival. Membership and
/// formation were mandatory for admission. Nonincrease remains separate from
/// local-cycle strictness: a call component may need it on an acyclic local hop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankArrivalEdge {
    pub source: SymbolHandle,
    pub destination: SymbolHandle,
    pub statement_ordinal: u32,
    pub target: TransitionTargetHandle,
    pub non_increasing: bool,
    pub strictly_decreases: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedMachineRankCorrespondence {
    pub machine: SymbolHandle,
    /// Exact authored body, distinguishing target-scoped bodies beside one
    /// callable symbol. These handles belong to the immutable typed program.
    pub body: HandleSpan<State>,
    pub entry_state: SymbolHandle,
    pub premise_regime: CheckedRankPremiseRegime,
    pub states: HandleSpan<CheckedStateRankCorrespondence>,
    pub edges: HandleSpan<CheckedRankArrivalEdge>,
}

/// Source admission's body-local result. A diagnostic is retained as export
/// payload; it is never a premise. Progress consumes only the established
/// local result, and final checks report the source judgment without replay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankBodyJudgment {
    pub machine: SymbolHandle,
    pub body: HandleSpan<State>,
    pub local_terminates: bool,
    pub entry_rank_formed: bool,
    /// Measure metadata projected once from this admitted body and its chosen
    /// coordinates. Computation production supplies only executable roots.
    pub observations: HandleSpan<crate::CheckedStateNaturalRank>,
    pub diagnostic: String,
}

/// One exact authored body in the complete call-component decision. A symbol
/// alone cannot distinguish target-scoped bodies sharing a callable owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRuntimeRankMember {
    pub machine: SymbolHandle,
    pub body: HandleSpan<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRuntimeNaturalState {
    pub state: SymbolHandle,
    pub coordinate: CheckedRankCoordinateHandle,
    pub parameter_position: u32,
    pub primitive_type: typed_trees::types::PrimitiveType,
}

impl Default for CheckedRuntimeNaturalState {
    /// Arena's ZII dummy is not a component state or a selected coordinate.
    /// Its non-integer carrier cannot supply natural-rank evidence either.
    fn default() -> Self {
        Self {
            state: SymbolHandle::invalid(),
            coordinate: CheckedRankCoordinateHandle::invalid(),
            parameter_position: 0,
            primitive_type: typed_trees::types::PrimitiveType::Bool,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRuntimeRankEdge {
    pub source: SymbolHandle,
    pub target: SymbolHandle,
    pub statement_ordinal: u32,
    pub strict: bool,
}

/// Complete source admission, retained once after body arrival admission.
/// An empty diagnostic establishes the component's shared edge ranking.
/// Natural observation rows are a projection of that decision, never a second
/// source proof. Reservations or a topology-only component supply no row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRuntimeRankComponent {
    pub members: HandleSpan<CheckedRuntimeRankMember>,
    pub states: HandleSpan<CheckedRuntimeNaturalState>,
    pub edges: HandleSpan<CheckedRuntimeRankEdge>,
    pub diagnostic: String,
}

/// Natural ranking of logical source invocations. These coordinates are
/// immutable machine Value/Const operands, never executable state slots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSourceNaturalRankMember {
    pub machine: SymbolHandle,
    pub body: HandleSpan<State>,
    pub subject: ExpressionHandle,
    pub carrier: TypeReferenceHandle,
    /// Exact original Value binder, when realized as ordinary state formals.
    /// These remain source rank coordinates, never executable slot claims.
    pub realized_from: Option<SymbolHandle>,
    pub realized_parameters: Vec<(SymbolHandle, SymbolHandle)>,
}

/// Every exact recurrent source invocation owes strict descent independently
/// of the theorem it cites. Call ordinals use the ordinary occurrence owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSourceNaturalRankEdge {
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub source_callee: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub call_ordinal: u32,
    pub authored_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    pub argument_position: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSourceNaturalRankComponent {
    pub members: HandleSpan<CheckedSourceNaturalRankMember>,
    pub edges: HandleSpan<CheckedSourceNaturalRankEdge>,
    pub view: language_semantics::RankingViewId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRankArrivalPlan {
    pub judgments: Arena<CheckedRankBodyJudgment>,
    pub machines: Arena<CheckedMachineRankCorrespondence>,
    pub states: Arena<CheckedStateRankCorrespondence>,
    pub bindings: Arena<CheckedRankCoordinateBinding>,
    pub coordinates: Arena<CheckedRankCoordinate>,
    pub path_segments: Arena<PlaceSegment>,
    pub transports: Arena<CheckedRankCoordinateTransport>,
    pub receiving: Arena<CheckedRankReceivingCoordinate>,
    pub captures: Arena<CheckedRankLocalCapture>,
    pub arrival_edges: Arena<CheckedRankArrivalEdge>,
    pub observations: Arena<crate::CheckedStateNaturalRank>,
    pub runtime_components: Arena<CheckedRuntimeRankComponent>,
    pub runtime_members: Arena<CheckedRuntimeRankMember>,
    pub runtime_states: Arena<CheckedRuntimeNaturalState>,
    pub runtime_edges: Arena<CheckedRuntimeRankEdge>,
    pub source_natural_components: Arena<CheckedSourceNaturalRankComponent>,
    pub source_natural_members: Arena<CheckedSourceNaturalRankMember>,
    pub source_natural_edges: Arena<CheckedSourceNaturalRankEdge>,
}

impl CheckedRankArrivalPlan {
    pub fn judgment_for_body(
        &self,
        machine: SymbolHandle,
        body: HandleSpan<State>,
    ) -> Option<&CheckedRankBodyJudgment> {
        let mut rows = self
            .judgments
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.machine == machine && row.body == body);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }

    /// Consume one exact body witness. A duplicate row is unavailable rather
    /// than a choice of whichever body's selection was published first.
    pub fn for_body(
        &self,
        machine: SymbolHandle,
        body: HandleSpan<State>,
    ) -> Option<&CheckedMachineRankCorrespondence> {
        let mut witnesses = self
            .machines
            .iter()
            .map(|(_, witness)| witness)
            .filter(|witness| witness.machine == machine && witness.body == body);
        let witness = witnesses.next()?;
        witnesses.next().is_none().then_some(witness)
    }
}
