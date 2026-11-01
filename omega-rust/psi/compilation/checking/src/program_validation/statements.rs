//! Ordered statement checks and flow-sensitive value updates:
//! `validate_state_statement_node` bundles what a check reads and writes,
//! then hands each statement kind to its owner beside this file.

mod assembly_fact_statements;
mod assignment_statements;
mod atomic_establishment;
mod call_statements;
mod expression_statements;
mod integer_establishment;
mod local_data_statements;
pub(super) use local_data_statements::validate_type_reference as validate_local_type_reference;
mod transition_statements;

use crate::value_custody::expression_types::{
    self, ExpressionTypeOwner, validate_expression_type_handle,
};
use crate::value_custody::locals::WritableRoots;
use crate::{
    TopLevelSymbols, ValidatedBoundaryOperatorApplication, declarations::transitions,
    proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::access_views, value_custody::reference_access, value_custody::struct_literals,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;
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

impl<'a> StatementScope<'a> {
    /// The read-side fields every statement kind's entry binds: the program,
    /// the machine and state under validation, and the caller's symbol,
    /// writable-root and transition environments. A module chooses the binding
    /// name for each position, and the kinds that need `statement_index` or
    /// the write-origin tables read them off the scope instead of re-spelling
    /// this list.
    pub(super) fn reads(
        &self,
    ) -> (
        &'a dyn crate::CheckingInput,
        &'a typed_trees::machine::Machine,
        &'a str,
        Option<&'a typed_trees::state::State>,
        &'a TopLevelSymbols<'a>,
        &'a WritableRoots<'a, 'a>,
    ) {
        (
            self.program,
            self.machine,
            self.state_name,
            self.current_state,
            self.symbols,
            self.writable_roots,
        )
    }
}

/// What every value that completes a state owes the state's declared result.
/// A trailing value is parsed as an unconditional value arm, so the terminal
/// expression and an authored `_ -> (value)` arm are one construct judged by
/// one roster.
///
/// The delivered-type check names the type that arrived. The syntactic shape
/// gate speaks only when that check is silent, and the class, nominal and
/// domain-weakening checks speak only when the gate is silent too: one defect
/// is one diagnostic. The gate is what compares a literal, call or cast with a
/// rigid type parameter, so a generic body is judged against its declared
/// binders whether or not any specialization of it exists.
pub(super) fn validate_completion_value(
    scope: &StatementScope<'_>,
    state: &typed_trees::state::State,
    value: ExpressionHandle,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (program, machine, state_name, _, _, _) = scope.reads();
    let return_owner = format!(
        "machine `{}` state `{state_name}` return value",
        machine.name
    );
    let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
    reference_access::report_reference_access_conflict(
        program,
        machine,
        Some(state),
        &program.statement_table.statements(state.statement_nodes)[..scope.statement_index],
        value,
        state.return_type,
        &return_owner,
        diagnostics,
    );
    struct_literals::validate_array_literal_elements(
        program,
        machine,
        state,
        value,
        state.return_type,
        diagnostics,
        formation_obligations,
    );
    let delivered_mark = diagnostics.len();
    expression_types::report_delivered_type_mismatch(
        program,
        machine.symbol,
        value,
        state.return_type,
        &return_owner,
        diagnostics,
    );
    let shape_mark = diagnostics.len();
    if shape_mark == delivered_mark {
        validate_expression_type_handle(
            program,
            machine.symbol,
            value,
            state.return_type,
            diagnostics,
            ExpressionTypeOwner::StateTerminalExpression {
                machine: machine.name.as_str(),
                state: state_name,
            },
        );
    }
    if diagnostics.len() != shape_mark {
        return;
    }
    // The gate rejects a cross-class LITERAL result (`-> i32 { true }`) but
    // blanket-accepts place and name values, so a Boolean field returned from
    // an `-> i32` machine reaches the class check instead.
    if let Some(target) = program.primitive_type_reference(state.return_type) {
        expression_types::report_cross_class_store(
            program,
            Some(machine),
            Some(state),
            value,
            target,
            &slot_context,
            "return value",
            diagnostics,
        );
    }
    // Nominal guard (`-> Foo { self.bar }`), the array/scalar shape guard
    // (`-> i32 { self.xs }`) and the scalar/data one, in that order.
    expression_types::report_place_value_mismatches(
        program,
        machine,
        Some(state),
        value,
        state.return_type,
        &slot_context,
        "return value",
        diagnostics,
    );
    domain_weakening::validate_return_domain_weakening(
        program,
        machine,
        state,
        value,
        &slot_context,
        diagnostics,
    );
}

pub(super) fn validate_state_statement_node<'p>(
    program: &'p dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
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
        access_views::validate_statement(program, machine, state, statement, diagnostics);
    }
    let scope = StatementScope {
        program,
        machine,
        state_name,
        current_state,
        statement_index,
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
    if outputs.diagnostics.len() > diagnostic_mark
        && let Some(source_span) = statement_source_span(program, statement)
    {
        for diagnostic in &mut outputs.diagnostics[diagnostic_mark..] {
            if diagnostic.source_span.is_none() {
                diagnostic.source_span = Some(source_span);
            }
        }
    }
}

/// The authored position a statement-level diagnostic belongs to. Variant
/// payloads retain their own spans or expressions worth pointing at; a
/// machine-free diagnostic reports no position rather than an invented one.
pub(crate) fn statement_source_span(
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
        StatementNode::AssemblyFact(fact) => {
            program.proof_fact_source_span(fact.fact).and_then(authored)
        }
        StatementNode::Assignment(assignment) => {
            expression_span(assignment.value).or_else(|| expression_span(assignment.target))
        }
        StatementNode::Call(call) => authored(call.source_span),
        StatementNode::Expression(expression) => expression_span(*expression),
        StatementNode::LocalData(local) => expression_span(local.initial_value),
        StatementNode::Transition(transition) => authored(transition.source_span),
    }
}
