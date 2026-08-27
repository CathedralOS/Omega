use crate::context::*;
mod lookup;
mod traversal;

use crate::lookup::machine_by_symbol;
use traversal::{CallSiteTraversal, find_call_site_in_statement};

pub(crate) enum CallSite<'program> {
    Statement(&'program psi_typed_trees::statement::TableCall),
    Expression {
        expression: ExpressionHandle,
        call: &'program psi_typed_trees::expression::TableCallExpression,
    },
    TransitionNamed {
        path: &'program psi_typed_trees::statement::TableNamePath,
        arguments: psi_arena::HandleSpan<ExpressionHandle>,
        evidence_arguments: &'program [Identifier],
        source_span: psi_source::SourceSpan,
        authored_call_selection: Option<
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
}

pub(crate) fn find_call_site<'program>(
    program: &'program psi_typed_trees::TypedTrees,
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

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program psi_typed_trees::TypedTrees,
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
