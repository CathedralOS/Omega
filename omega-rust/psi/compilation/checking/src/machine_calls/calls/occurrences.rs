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
    /// Logical invocation at the same executable occurrence/ordinal. This
    /// reads the original payload; it neither selects a body nor grants its
    /// source application's argument, contract or effect admission.
    pub fn source(self, program: &'program dyn crate::CheckingInput) -> Option<Self> {
        match self {
            Self::Expression { expression, call } => Some(Self::Expression {
                expression,
                call: program.expression_table.source_call(call)?,
            }),
            other => Some(other),
        }
    }

    /// Use the flow-resolved endpoint for an unchanged ordinary call. A
    /// redirected call must instead name its retained original endpoint; an
    /// unresolved original cannot borrow its private provider's authority.
    pub fn source_target_symbol(
        self,
        program: &'program dyn crate::CheckingInput,
        resolved_target: SymbolHandle,
    ) -> Option<SymbolHandle> {
        if let Self::Expression { expression, .. } = self {
            let executable = program.expression_table.call_node(expression)?;
            if executable.source_call.is_valid() {
                let target = program.expression_table.source_call(executable)?.target_symbol;
                return target.is_valid().then_some(target);
            }
        }
        Some(resolved_target)
    }

    pub fn target_symbol(&self) -> SymbolHandle {
        match self {
            Self::Statement(call) => call.target_symbol,
            Self::Expression { call, .. } => call.target_symbol,
            Self::TransitionNamed { path, .. } => path.symbol,
        }
    }

    pub fn static_requirement_dispatch(
        &self,
    ) -> Option<&typed_trees::typed_trees::StaticRequirementDispatch> {
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

/// Source judgment projection of the same exact executable occurrence. This
/// shares ordinal discovery; it does not walk or check another invocation.
pub fn find_source_call_site<'program>(
    program: &'program dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    find_call_site(program, machine_symbol, state_symbol, statement_index, call_ordinal)?
        .source(program)
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
    let mut traversal =
        CallSiteTraversal::new(program, machine, state, statement_index, call_ordinal);
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
    let mut traversal =
        CallSiteTraversal::new(program, machine, state, statement_index, call_ordinal);
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
    let mut traversal =
        CallSiteTraversal::collecting(program, machine, state, statement_index, &mut collected);
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
    let mut traversal =
        CallSiteTraversal::new(program, machine, state, statement_index, call_ordinal);
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
