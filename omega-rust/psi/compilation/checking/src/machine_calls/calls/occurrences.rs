//! Authored callable occurrences in one statement's deterministic ordinal order.
//! This executable walk visits Atomic.value only, deduplicates DAG call handles,
//! and retains conditional ancestry without pruning authored alternatives.

use super::topology::find_state_in_machine;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::name::Identifier;
use typed_trees::statement::StatementNode;
mod traversal;
use traversal::{CallSiteTraversal, find_call_site_in_statement};

#[derive(Clone, Copy)]
pub enum CallSite<'program> {
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

impl<'program> CallSite<'program> {
    /// A named edge is an internal arrival only when its exact target is a
    /// state owned by this invocation. A machine target remains an invocation,
    /// including a normalized recursive tail call; syntax and marker bits do
    /// not remove its contract or erased evidence lane.
    pub fn is_internal_state_transfer(
        &self,
        program: &dyn crate::CheckingInput,
        caller_machine: SymbolHandle,
    ) -> bool {
        matches!(self, Self::TransitionNamed { path, .. }
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::State
                && program.symbols.get(path.symbol).parent == caller_machine)
    }

    pub fn static_requirement_dispatch(
        &self,
    ) -> Option<&'program typed_trees::typed_trees::StaticRequirementDispatch> {
        match self {
            Self::Statement(call) => call.static_requirement_dispatch.as_ref(),
            Self::Expression { call, .. } => call.static_requirement_dispatch.as_ref(),
            Self::TransitionNamed { .. } => None,
        }
    }
}

pub fn find_call_site<'program>(
    program: &'program dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = program.machine_by_symbol(machine_symbol)?;
    find_call_site_in_state(program, machine, state, statement_index, call_ordinal)
}

/// Exact ordinary call occurrence that executes without a selection fork.
/// A call inventory includes alternatives; it is not evidence that their
/// transfers discharge unconditional state-local disposal. This query shares
/// lookup's traversal and ordinals, retaining its selection ancestry instead.
/// Calls after an earlier state transition require edge-specific custody.
pub fn call_is_unconditional(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<()> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = program.machine_by_symbol(machine_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if statements
        .get(..statement_index)?
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return None;
    }
    let statement = statements.get(statement_index)?;
    let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
    find_call_site_in_statement(&mut traversal, statement)?;
    (!traversal.matched_conditionally).then_some(())
}

/// Call-site resolution against an already-located machine/state pair, for
/// callers that own the symbols' storage (e.g. every checked body call of one
/// state) and must not rescan the program per call.
pub fn find_call_site_in_state<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
    find_call_site_in_statement(&mut traversal, statement)
}

/// One traversal over a statement recording every call site in ordinal order.
/// Batching per statement keeps per-call work off quadratic re-walks when a
/// caller (checked-body source binding) needs every site anyway.
pub fn collect_call_sites_in_statement<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
) -> Vec<CallSite<'program>> {
    let mut collected = Vec::new();
    let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
        return collected;
    };
    let mut traversal = CallSiteTraversal::collecting(program, machine, state, &mut collected);
    let _ = find_call_site_in_statement(&mut traversal, statement);
    collected
}

/// Reuse call-ordinal traversal to distinguish guard evaluation from the two
/// mutually exclusive target operands of a transition.
pub fn transition_call_target(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<typed_trees::statement::TransitionTargetHandle> {
    let transition = program
        .statement_table
        .statement_at(state, statement_index)?
        .as_transition()?;
    let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
    traversal::transition_call_target(&mut traversal, transition)
}

pub fn call_site_argument_expressions<'program>(
    program: &'program dyn crate::CheckingInput,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression { call, .. } => {
            program.expression_table.expression_handles(call.arguments)
        }
        CallSite::TransitionNamed { arguments, .. } => {
            program.statement_table.expression_handles(*arguments)
        }
    }
}

pub fn call_site_evidence_arguments<'program>(
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
