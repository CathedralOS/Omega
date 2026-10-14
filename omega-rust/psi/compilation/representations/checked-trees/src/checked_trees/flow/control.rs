use arena::HandleSpan;
use language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
use symbols::SymbolHandle;

use crate::{BorrowArgumentAccessFact, BorrowWritableRootFact, ContractProofFactRef};

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowConstraintRef,
    FlowInvalidationFact, FlowSemanticContextRef,
};

/// Preconditions at a spelled, implicit, or named operator invocation, after
/// operand evaluation and before either comparison branch acquires its own
/// observations. A spelled or Match-comparison use names its `operator_use`
/// row; a named `Namespace::requirement(...)` call has no `uses` row, so it
/// names its `named_use` row instead. Exactly one of the two handles is
/// valid.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOperatorInvocationFact {
    pub operator_use: arena::Handle<crate::CheckedOperatorUseFact>,
    pub named_use: arena::Handle<crate::CheckedNamedOperatorUseFact>,
    pub operands: HandleSpan<super::FlowOperandFact>,
    pub requires_constraints: HandleSpan<FlowConstraintRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    /// Actuals in authored evaluation order, captured at their own completion.
    pub operands: HandleSpan<super::FlowOperandFact>,
    /// Exact expression occurrence captured by the semantic call traversal.
    /// Statement calls and named transition roots use the zero handle.
    pub authored_expression: typed_trees::expression::ExpressionHandle,
    /// Which authored syntax spells this invocation, bound with
    /// `authored_target` and `authored_arguments` by the source-custody
    /// binding pass. Lowering emits the call from these rows instead of
    /// walking the statement again to find its occurrence.
    pub authored_site: FlowCallSite,
    /// Callee identity as spelled: the call's target symbol, a named
    /// `Namespace::requirement(...)` call's selected boundary declaration, or
    /// a named transition's path. `target_symbol` is the resolved state.
    pub authored_target: SymbolHandle,
    /// Explicit arguments in source order, without a receiver spelled before
    /// the call. Rows live in [`super::FlowControlFacts::call_arguments`].
    pub authored_arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
    /// The receiver a call expression spells before its name
    /// (`self.samples.element(2)`); the zero handle for every other site.
    pub authored_receiver: typed_trees::expression::ExpressionHandle,
    /// What the authored statement does with the call's result.
    pub authored_result: FlowCallResult,
    /// A named transition that is the state's only transition: its final,
    /// unconditional statement, with no continuation, an ordinary exit and a
    /// selected (not static-parameter) target. A linear operation roster may
    /// sequence exactly this invocation; a guarded arm's runs in its arm.
    pub authored_closing_invocation: bool,
    /// The part this occurrence plays in a closed-case dispatch whose subject
    /// is a call.
    pub authored_dispatch_role: FlowCallDispatchRole,
    /// Whether the occurrence runs only on a selected path of its statement:
    /// the right operand of `&&`/`||`, a match arm's pattern or value, or a
    /// transition's target or continuation other than an unguarded machine
    /// call. Such a call cannot retire a result every path still owes.
    pub authored_selected: bool,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    /// The closed static conformance selection the checker resolved for this
    /// occurrence, when `target_symbol` is a requirement realization reached
    /// through one. Lowering retains conformance executables and proof
    /// output targets from these rows; a named transition has none.
    pub static_requirement_dispatch: Option<crate::StaticRequirementDispatchFact>,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub requires_contexts: HandleSpan<FlowSemanticContextRef>,
    pub requires_constraints: HandleSpan<FlowConstraintRef>,
    pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub exit_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    /// Exact authored call location retained while the typed call identity is
    /// still stable. Provider settlement may later rewrite the typed call.
    pub authored_source_span: Option<source::SourceSpan>,
    /// Whether source custody was internally coherent at capture. Ordinary
    /// compilation need not require package-review provenance, but package
    /// projection rejects a false value.
    pub authored_source_custody_valid: bool,
}

impl FlowCallFact {
    /// Formal argument positions exclude the separately evaluated receiver.
    /// Keep the full evaluation roster for sequencing and expose this span
    /// for consumers joining explicit actuals to the callee telescope.
    pub fn argument_operands(&self) -> HandleSpan<super::FlowOperandFact> {
        if !self.authored_receiver.is_valid() || self.operands.is_empty() {
            return self.operands;
        }
        HandleSpan::from_parts_or_empty(self.operands.handle_at(1), self.operands.count() - 1)
    }
}

/// The authored syntax that spells one [`FlowCallFact`]. `Unbound` is the
/// absence state before the source-custody binding pass joins the row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowCallSite {
    #[default]
    Unbound,
    /// A call statement; the statement coordinate names the occurrence.
    Statement,
    /// A call expression at `authored_expression`.
    Expression,
    /// A named transition target invoking another machine.
    NamedTransition,
}

/// What the authored statement does with one call's result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowCallResult {
    /// The result feeds an enclosing expression, an assignment, a mutable
    /// local or a transition.
    #[default]
    Operand,
    /// A call statement that discards the value it produces.
    Discarded,
    /// The whole initializer of an immutable local, which names the result.
    ImmutableLocal,
}

/// The part one [`FlowCallFact`] plays in a closed-case dispatch over a
/// computed subject. The parser copies the dispatch's call into every arm's
/// guard and payload read; the language evaluates it once, at the first arm
/// (wiki/spec/language/expressions.md, "Transition dispatch").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowCallDispatchRole {
    /// A call outside any copy of a computed dispatch subject, including a
    /// call inside the evaluated subject itself.
    #[default]
    Ordinary,
    /// The computed subject's one evaluation, at the dispatch's first arm.
    SubjectEvaluation,
    /// A copy of the subject in a guard or payload read, or a call inside
    /// such a copy. It keeps the ordinal the checker numbers it with but
    /// reads the saved result: nothing runs and its arguments do not run
    /// again.
    SavedSubjectRead,
}

/// Coordinate of a [`FlowCallFact`] whose authored call selected execution
/// replaced in the typed body: the row is retained for the handles held on
/// it and skipped by execution planning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetiredFlowCall {
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStatementFact {
    pub statement_index: usize,
    /// Completed value before destination evaluation and storage. Zero for
    /// statements without a stored value; children retain their own epochs.
    pub stored_value: arena::Handle<super::FlowOperandFact>,
    /// Exact established destination in the semantic place arena. Dynamic or
    /// unresolved destinations cannot supply a reaching value capture.
    pub stored_destination: facts::PlaceHandle,
    /// Evaluation-order write prefix at statement completion, including its
    /// final store. Transition rows retain only the fallthrough guard effects.
    pub completed_writes: HandleSpan<super::FlowOperandReferent>,
    pub completed_unknown_write: bool,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    /// Live facts after this statement's mutations and value transfers, before
    /// any subsequent statement or proof output can restore an invariant.
    pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    /// Live observations after operand evaluation, before destination storage
    /// is invalidated or its replacement established. Replacement cleanup
    /// consumes the old value at this point, not at statement entry or exit.
    pub pre_store_constraints: HandleSpan<FlowConstraintRef>,
}

/// Live source observations after the selected outgoing operands have been
/// evaluated, before successor admission/arrival or machine-return guarantees.
/// Unlike `FlowExitFact`, this includes named state transfers and introduces
/// no machine result or ensures obligation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateOutgoingFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    /// Invalid only for implicit completion or a final expression.
    pub transition_target: typed_trees::statement::TransitionTargetHandle,
    /// Exact completed value on this outgoing occurrence. Zero for named
    /// transfers and valueless completion; reference results retain live
    /// referents captured during evaluation rather than replaying selectors.
    pub returned_value: arena::Handle<super::FlowOperandFact>,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub transition_target: typed_trees::statement::TransitionTargetHandle,
    /// Exact completed value on this outgoing occurrence. Zero for named
    /// transfers and valueless completion; reference results retain live
    /// referents captured during evaluation rather than replaying selectors.
    pub returned_value: arena::Handle<super::FlowOperandFact>,
    pub parameter_origins: HandleSpan<FlowExitParameterOrigin>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
    pub ensures_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowExitParameterOrigin {
    pub contract: arena::Handle<typed_trees::domain::ProofFact>,
    pub entry_parameter: SymbolHandle,
    /// Invalid when explicit incoming edges do not establish one exact origin.
    pub state_parameter: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub borrow_activations: HandleSpan<FlowBorrowActivationFact>,
    pub borrow_weakenings: HandleSpan<FlowBorrowWeakeningFact>,
    pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
    pub statements: HandleSpan<FlowStatementFact>,
    pub case_accesses: HandleSpan<super::FlowCaseAccessFact>,
    pub calls: HandleSpan<FlowCallFact>,
    pub exits: HandleSpan<FlowExitFact>,
    pub outgoing: HandleSpan<FlowStateOutgoingFact>,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
}
