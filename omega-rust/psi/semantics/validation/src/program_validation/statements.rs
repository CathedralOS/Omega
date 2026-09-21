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
    ExactIntegerCastFact, TopLevelSymbols, ValidatedBoundaryOperatorApplication,
    declarations::transitions, proof_contracts::arithmetic_domains, value_custody::placed_views,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

#[allow(clippy::too_many_arguments)]
/// What every statement check reads: the program, the machine and state
/// under validation, the symbol tables and the transition environments.
#[derive(Clone, Copy)]
pub(super) struct StatementScope<'a> {
    pub(super) program: &'a TypedTrees,
    pub(super) machine: &'a typed_trees::machine::Machine,
    pub(super) state_name: &'a str,
    pub(super) current_state: Option<&'a typed_trees::state::State>,
    pub(super) machine_symbols: &'a MachineSymbols<'a>,
    pub(super) symbols: &'a TopLevelSymbols<'a>,
    pub(super) writable_roots: &'a WritableRoots<'a, 'a>,
    pub(super) transition_values: &'a transitions::TransitionValueEnvironments,
}

/// What a statement check writes: the flow-sensitive value environment, the
/// facts it establishes, and diagnostics.
pub(super) struct StatementOutputs<'a> {
    pub(super) value_env: &'a mut arithmetic_domains::ValueEnv,
    pub(super) exact_integer_casts: &'a mut Vec<ExactIntegerCastFact>,
    pub(super) boundary_operator_applications: &'a mut Vec<ValidatedBoundaryOperatorApplication>,
    pub(super) diagnostics: &'a mut Vec<Diagnostic>,
}

pub(super) fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement_handle: typed_trees::statement::StatementHandle,
    statement: &StatementNode,
    value_env: &mut arithmetic_domains::ValueEnv,
    transition_values: &transitions::TransitionValueEnvironments,
    direct_written: Option<Vec<String>>,
    exact_integer_casts: &mut Vec<ExactIntegerCastFact>,
    boundary_operator_applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = current_state {
        placed_views::validate_statement(program, machine, state, statement, diagnostics);
    }
    let scope = StatementScope {
        program,
        machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        transition_values,
    };
    let mut outputs = StatementOutputs {
        value_env,
        exact_integer_casts,
        boundary_operator_applications,
        diagnostics,
    };
    match statement {
        StatementNode::RootBinding(_) => {}
        StatementNode::AssemblyFact(_) => {
            assembly_fact_statements::validate(&scope, &mut outputs, statement)
        }
        StatementNode::Assignment(_) => {
            assignment_statements::validate(&scope, &mut outputs, statement, direct_written)
        }
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
}
