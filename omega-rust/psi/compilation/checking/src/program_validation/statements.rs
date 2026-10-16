//! Ordered statement checks and flow-sensitive value updates:
//! `validate_state_statement_node` bundles what a check reads and writes,
//! then hands each statement kind to its owner beside this file.

mod assembly_fact_statements;
mod assignment_statements;
mod call_statements;
mod expression_statements;
mod local_data_statements;
mod transition_statements;

use crate::declarations::symbols::MachineSymbols;
use crate::value_custody::locals::WritableRoots;
use crate::{
    TopLevelSymbols, ValidatedBoundaryOperatorApplication, declarations::transitions,
    proof_contracts::arithmetic_domains, value_custody::placed_views,
};
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;

#[allow(clippy::too_many_arguments)]
/// What every statement check reads: the program, the machine and state
/// under validation, the symbol tables and the transition environments.
#[derive(Clone, Copy)]
pub(super) struct StatementScope<'a> {
    pub(super) program: &'a dyn crate::CheckingInput,
    pub(super) machine: &'a typed_trees::machine::Machine,
    pub(super) state_name: &'a str,
    pub(super) current_state: Option<&'a typed_trees::state::State>,
    pub(super) statement_index: usize,
    pub(super) machine_symbols: &'a MachineSymbols<'a>,
    pub(super) symbols: &'a TopLevelSymbols<'a>,
    pub(super) writable_roots: &'a WritableRoots<'a, 'a>,
    pub(super) local_write_origins: Option<&'a [crate::LocalWriteOrigin]>,
    pub(super) call_frames: Option<&'a crate::CallFrameResolver<'a>>,
    pub(super) transition_values: &'a transitions::TransitionValueEnvironments,
}

/// What a statement check writes: the flow-sensitive value environment, the
/// facts it establishes, and diagnostics.
pub(super) struct StatementOutputs<'a> {
    pub(super) stored_integer_writes: &'a mut Vec<checked_trees::CheckedStoredIntegerWrite>,
    pub(super) value_environment: &'a mut arithmetic_domains::ValueEnvironment,
    pub(super) arithmetic_decisions: &'a mut arithmetic_domains::ArithmeticDecisions,
    pub(super) boundary_operator_applications: &'a mut Vec<ValidatedBoundaryOperatorApplication>,
    pub(super) diagnostics: &'a mut Vec<Diagnostic>,
}

pub(super) fn validate_state_statement_node<'p>(
    program: &'p dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement_handle: typed_trees::statement::StatementHandle,
    statement_index: usize,
    statement: &StatementNode,
    value_environment: &mut arithmetic_domains::ValueEnvironment,
    transition_values: &transitions::TransitionValueEnvironments,
    direct_written: Option<Vec<String>>,
    local_write_origins: Option<&[crate::LocalWriteOrigin]>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    arithmetic_decisions: &mut arithmetic_domains::ArithmeticDecisions,
    stored_integer_writes: &mut Vec<checked_trees::CheckedStoredIntegerWrite>,
    boundary_operator_applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    bound_lookup: &mut (
        &'p dyn crate::CheckingInput,
        Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>>,
    ),
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic_mark = diagnostics.len();
    if let Some(state) = current_state {
        placed_views::validate_statement(program, machine, state, statement, diagnostics);
    }
    let scope = StatementScope {
        program,
        machine,
        state_name,
        current_state,
        statement_index,
        machine_symbols,
        symbols,
        writable_roots,
        transition_values,
        local_write_origins,
        call_frames,
    };
    let mut outputs = StatementOutputs {
        stored_integer_writes,
        value_environment,
        arithmetic_decisions,
        boundary_operator_applications,
        diagnostics,
    };
    match statement {
        StatementNode::RootBinding(_) => {}
        StatementNode::Assembly(_) => {} // Control admission owns the lexical window and local edges.
        StatementNode::AssemblyFact(_) => {
            assembly_fact_statements::validate(&scope, &mut outputs, statement)
        }
        StatementNode::Assignment(_) => assignment_statements::validate(
            &scope,
            &mut outputs,
            statement,
            direct_written,
            bound_lookup,
        ),
        StatementNode::Call(_) => call_statements::validate(
            &scope,
            &mut outputs,
            statement,
            statement_handle,
            direct_written,
        ),
        StatementNode::Expression(_) => {
            expression_statements::validate(&scope, &mut outputs, statement)
        }
        StatementNode::LocalData(_) => {
            local_data_statements::validate(&scope, &mut outputs, statement)
        }
        StatementNode::Transition(_) => {
            transition_statements::validate(&scope, &mut outputs, statement)
        }
    }
    // A check mints only message text; the statement under validation owns
    // the authored position every consumer attributes it to.
    if let Some(source_span) = statement_source_span(program, statement) {
        for diagnostic in &mut diagnostics[diagnostic_mark..] {
            if diagnostic.source_span.is_none() {
                diagnostic.source_span = Some(source_span);
            }
        }
    }
}

/// The authored position a statement-level diagnostic belongs to. Variant
/// payloads retain their own spans or expressions worth pointing at; a
/// machine-free diagnostic reports no position rather than an invented one.
pub(super) fn statement_source_span(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
) -> Option<source::SourceSpan> {
    // Synthesized records retain the default span; reporting it would point
    // at the map's first file. An absent span defers to the declaration's
    // own provenance instead.
    let authored = |source_span: source::SourceSpan| -> Option<source::SourceSpan> {
        (source_span != source::SourceSpan::default()).then_some(source_span)
    };
    let expression_span = |handle: typed_trees::expression::ExpressionHandle| {
        handle
            .is_valid()
            .then(|| program.expression_table.source_span(handle))
            .and_then(authored)
    };
    match statement {
        StatementNode::Assembly(_) => None,
        StatementNode::RootBinding(binding) => authored(binding.source_span),
        StatementNode::AssemblyFact(fact) => expression_span(fact.expression),
        StatementNode::Assignment(assignment) => {
            expression_span(assignment.value).or_else(|| expression_span(assignment.target))
        }
        StatementNode::Call(call) => authored(call.source_span),
        StatementNode::Expression(expression) => expression_span(*expression),
        StatementNode::LocalData(local) => expression_span(local.initial_value),
        StatementNode::Transition(transition) => authored(transition.source_span),
    }
}
