use checked_trees::expression::ExpressionHandle;
use checked_trees::name::Identifier;
use checked_trees::statement::StatementNode;
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::SymbolHandle;
mod lookup;
mod traversal;

use crate::lookup::machine_by_symbol;
use traversal::{CallSiteTraversal, find_call_site_in_statement};

#[derive(Clone, Copy)]
pub(crate) enum CallSite<'program> {
    Statement(&'program typed_trees::statement::TableCall),
    Expression {
        expression: ExpressionHandle,
        call: &'program typed_trees::expression::TableCallExpression,
    },
    TransitionNamed {
        path: &'program typed_trees::statement::TableNamePath,
        arguments: arena::HandleSpan<ExpressionHandle>,
        evidence_arguments: &'program [Identifier],
        source_span: source::SourceSpan,
        authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
}

impl CallSite<'_> {
    pub(crate) fn static_requirement_dispatch(
        &self,
    ) -> Option<&typed_trees::typed_trees::StaticRequirementDispatch> {
        match self {
            Self::Statement(call) => call.static_requirement_dispatch.as_ref(),
            Self::Expression { call, .. } => call.static_requirement_dispatch.as_ref(),
            Self::TransitionNamed { .. } => None,
        }
    }
}

/// The compiler-owned collection or text view an authored call selects, or
/// `None` when the call is not a view.
///
/// A collection-view call is authored as a bare method spelling on a carrier
/// (`cells.as_slice()`, `text.as_view()`, `view.bytes()`). The language owns
/// the operation, no package declares it, and the result observes the
/// receiver's existing storage instead of producing a new value. The spelling
/// alone never establishes that: it is compared through
/// [`CollectionViewOperation::from_authored_spelling`], and the call shape
/// decides. Each condition rejects a different operation which happens to
/// share the name:
///
/// * `target_symbol` must be invalid. A resolved nominal machine spelled
///   `as_slice` is that machine's declared operation; its result may be any
///   projection of anything it can reach, so evidence taken from the receiver
///   would not describe it.
/// * `receiver` must be valid. A view needs the carrier it views.
/// * the call must carry no arguments, evidence arguments, machine arguments,
///   static requirement dispatch, quotient operation or private layout
///   operation. Every one of those names an operand or a dispatch decision
///   that a compiler-owned view does not accept, so a call carrying one is a
///   different operation regardless of how it is spelled.
///
/// The selected operation is returned rather than a boolean: a caller which
/// only claims element views of a collection (the ownership and projected
/// transfer lanes) matches on the exact operations it claims, keeping that
/// narrowing visible at its own site.
///
/// Checking records the operation once as
/// `AuthoredDeclarationSelectionIntrinsic::CollectionView`; consumers holding
/// the selection ledger read the retained identity instead of asking here.
pub(crate) fn collection_view_call(
    program: &typed_trees::TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<CollectionViewOperation> {
    if call.target_symbol.is_valid()
        || !call.receiver.is_valid()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        || !call.evidence_arguments.is_empty()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    CollectionViewOperation::from_authored_spelling(call.target.as_str())
}

pub(crate) fn find_call_site<'program>(
    program: &'program typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = machine_by_symbol(program, machine_symbol)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let mut current_ordinal = 0usize;
    let mut traversal = CallSiteTraversal::new(
        program,
        machine,
        state,
        statement_index,
        statement_index,
        call_ordinal,
        &mut current_ordinal,
    );
    find_call_site_in_statement(&mut traversal, statement)
}

/// Reuse call-ordinal traversal to distinguish guard evaluation from the two
/// mutually exclusive target operands of a transition.
pub(crate) fn transition_call_target(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<typed_trees::statement::TransitionTargetHandle> {
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    let mut current_ordinal = 0;
    let mut traversal = CallSiteTraversal::new(
        program,
        machine,
        state,
        statement_index,
        statement_index,
        call_ordinal,
        &mut current_ordinal,
    );
    traversal::transition_call_target(&mut traversal, transition)
}

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    lookup::call_site_argument_expressions(program, call_site)
}

pub(crate) fn call_site_evidence_arguments<'program>(
    call_site: &CallSite<'program>,
) -> &'program [Identifier] {
    match call_site {
        CallSite::Statement(call) => &call.evidence_arguments,
        CallSite::Expression { call, .. } => &call.evidence_arguments,
        CallSite::TransitionNamed {
            evidence_arguments, ..
        } => evidence_arguments,
    }
}

pub(crate) use lookup::{
    call_target_parameters, call_target_type_parameters, find_machine, find_machine_by_entry_state,
    find_machine_head, find_state, find_state_in_machine, find_state_with_machine,
};
