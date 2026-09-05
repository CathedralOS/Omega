use crate::context::*;
mod lookup;
mod traversal;

use crate::lookup::machine_by_symbol;
use traversal::{CallSiteTraversal, find_call_site_in_statement};

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
    call_target_parameters, call_target_type_parameters, find_state, find_state_in_machine,
};
