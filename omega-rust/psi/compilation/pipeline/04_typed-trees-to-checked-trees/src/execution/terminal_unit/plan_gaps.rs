//! Join a machine's missing executable plan to the authored source.
//!
//! A gap names the construct plan construction could not lower, or the call
//! whose callee has no body, at the statement, state or machine where it sits.
//! The span and coverage form chosen here are what `omega --check` reports
//! when lowering or entry establishment demands that body; an undemanded gap
//! (a proof lemma over `Nat`, an unused library body) is never reported.

use checked_trees::{CheckedUnitPlanGap, CheckedUnitPlanGapReason};
use diagnostics::CoverageForm;
use source::{SourceSpan, Span};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

/// Where in a machine's body a gap sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GapSite {
    /// The machine as a whole: its declaration or signature.
    Machine,
    /// One authored state's signature or its cleanup obligations.
    State(SymbolHandle),
    /// One statement, by its position in `state`'s statement list.
    Statement { state: SymbolHandle, index: u32 },
}

pub(crate) fn plan_gap(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    site: GapSite,
    reason: CheckedUnitPlanGapReason,
) -> CheckedUnitPlanGap {
    let declaration = |symbol: SymbolHandle| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .unwrap_or_default()
    };
    let machine_span = declaration(machine);
    let (coverage, source_span) = match site {
        GapSite::Machine => (CoverageForm::MachineHeader, machine_span),
        GapSite::State(state) => (
            CoverageForm::StateSignature,
            authored_or(declaration(state), machine_span),
        ),
        GapSite::Statement { state, index } => {
            let state_span = authored_or(declaration(state), machine_span);
            match program.state_by_symbol(state).and_then(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(index as usize)
            }) {
                Some(statement) => {
                    let (coverage, span) = statement_site(program, statement);
                    (coverage, authored_or(span, state_span))
                }
                None => (CoverageForm::StatementForm, state_span),
            }
        }
    };
    CheckedUnitPlanGap {
        machine,
        state: match site {
            GapSite::Machine => SymbolHandle::default(),
            GapSite::State(state) | GapSite::Statement { state, .. } => state,
        },
        statement_index: match site {
            GapSite::Statement { index, .. } => Some(index),
            _ => None,
        },
        reason,
        source_span,
        coverage,
    }
}

/// The coverage form and authored extent of one statement.
fn statement_site(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> (CoverageForm, SourceSpan) {
    let expression = |handle: ExpressionHandle| {
        if handle.is_valid() {
            program.expression_table.source_span(handle)
        } else {
            SourceSpan::default()
        }
    };
    match statement {
        StatementNode::Assembly(_) => (CoverageForm::StatementForm, SourceSpan::default()),
        StatementNode::Call(call) => (CoverageForm::CallForm, call.source_span),
        StatementNode::Transition(transition) => {
            (CoverageForm::TransitionForm, transition.source_span)
        }
        StatementNode::RootBinding(binding) => (CoverageForm::StatementForm, binding.source_span),
        StatementNode::Assignment(assignment) => (
            CoverageForm::StatementForm,
            joined(expression(assignment.target), expression(assignment.value)),
        ),
        StatementNode::LocalData(local) => (
            CoverageForm::StatementForm,
            joined(
                program
                    .symbols
                    .symbol_source_span(local.symbol)
                    .unwrap_or_default(),
                expression(local.initial_value),
            ),
        ),
        StatementNode::Expression(handle) => (CoverageForm::ExpressionForm, expression(*handle)),
        StatementNode::AssemblyFact(fact) => (
            CoverageForm::StatementForm,
            program
                .proof_fact_source_span(fact.fact)
                .unwrap_or_default(),
        ),
    }
}

fn is_authored(span: SourceSpan) -> bool {
    span != SourceSpan::default()
}

fn authored_or(span: SourceSpan, fallback: SourceSpan) -> SourceSpan {
    if is_authored(span) { span } else { fallback }
}

/// The extent covering both spans when they share a source; otherwise the
/// first authored one.
fn joined(first: SourceSpan, second: SourceSpan) -> SourceSpan {
    match (is_authored(first), is_authored(second)) {
        (true, true) if first.source_id == second.source_id => SourceSpan::new(
            first.source_id,
            Span::new(
                first.span.start.min(second.span.start),
                first.span.end.max(second.span.end),
            ),
        ),
        (false, true) => second,
        _ => first,
    }
}
