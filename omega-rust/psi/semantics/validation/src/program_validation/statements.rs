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

pub(super) fn is_exact_executable_drop_body(
    program: &TypedTrees,
    cleanup: &typed_trees::machine::Machine,
) -> bool {
    let [cleanup_state] = program.machine_states(cleanup) else {
        return false;
    };
    let statements = program
        .statement_table
        .statements(cleanup_state.statement_nodes);
    if statements.is_empty() {
        return false;
    }

    let mut helper_symbols = Vec::with_capacity(statements.len());
    for statement in statements {
        let StatementNode::Call(call) = statement else {
            return false;
        };
        if !call.machine_arguments.is_empty()
            || !program
                .statement_table
                .expression_handles(call.arguments)
                .is_empty()
            || call.discards_result
        {
            return false;
        }

        let helpers = program
            .machines()
            .iter()
            .filter_map(|machine| {
                let [state] = program.machine_states(machine) else {
                    return None;
                };
                (state.symbol == call.target_symbol).then_some((machine, state))
            })
            .collect::<Vec<_>>();
        let [(helper, helper_state)] = helpers.as_slice() else {
            return false;
        };
        let Some(helper_attachment) = helper.attached_data.as_ref() else {
            return false;
        };
        let helper_data = program
            .data_definitions()
            .iter()
            .filter(|data| &data.name == helper_attachment)
            .collect::<Vec<_>>();
        let [helper_data] = helper_data.as_slice() else {
            return false;
        };

        if helper.symbol == cleanup.symbol
            || helper_symbols.contains(&helper.symbol)
            || helper.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !helper.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(helper).is_empty()
            || !program.machine_owned_data(helper).is_empty()
            || !program.machine_trait_conformances(helper).is_empty()
            || !helper.conformance_bounds.is_empty()
            || !program.machine_invokes(helper).is_empty()
            || helper.suspends
            || helper.blocks
            || !program.machine_contracts(helper).is_empty()
            || !program.state_parameters(helper_state).is_empty()
            || !program.state_contracts(helper_state).is_empty()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(helper_state.return_type),
                typed_trees::types::TypeReferenceNode::Unit
            )
            || !program
                .statement_table
                .statements(helper_state.statement_nodes)
                .is_empty()
            || !helper_data.lifetime_parameters.is_empty()
            || !program.data_type_parameters(helper_data).is_empty()
            || !program.data_members(helper_data).is_empty()
        {
            return false;
        }
        helper_symbols.push(helper.symbol);
    }
    true
}

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
