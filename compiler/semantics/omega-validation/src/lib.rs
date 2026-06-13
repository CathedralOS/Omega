mod calls;
mod contract_entailment;
mod data;
mod domains;
mod effects;
mod entry_point;
mod expression_types;
mod invariants;
mod locals;
mod machine_data;
mod operators;
mod places;
mod proof_facts;
mod properties;
mod state_signatures;
mod struct_literals;
mod symbols;
#[cfg(test)]
mod tests;
mod traits;
mod transitions;
mod type_references;
mod wire;

use crate::calls::{validate_call_node, validate_value_position_calls};
use crate::contract_entailment::validate_machine_contract_entailment;
use crate::data::validate_data_field_types;
use crate::domains::validate_domain_definitions;
use crate::entry_point::validate_entry_point;
use crate::expression_types::{ExpressionTypeOwner, validate_expression_type_handle};
use crate::invariants::validate_invariant_definitions;
use crate::locals::{WritableRoots, validate_local_data_names};
use crate::machine_data::{
    validate_contained_types, validate_owned_data, validate_version_scoped_target,
};
use crate::places::validate_assignment_target_handle;
use crate::state_signatures::{
    StateSignatureOwner, validate_callable_state_signatures, validate_machine_contracts,
    validate_machine_effects,
};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use crate::traits::{
    validate_data_conformances, validate_machine_trait_conformances, validate_trait_requirements,
};
use crate::transitions::validate_transition_target_node;
use crate::type_references::{TypeReferenceOwner, validate_type_reference_handle};
pub use effects::validate_effect_plan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::statement::StatementNode;

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = omega_facts::build_definition_fact_plan(program);

    validate_domain_definitions(program, &symbols, &fact_plan, &mut diagnostics);
    validate_invariant_definitions(program, &fact_plan, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_trait_requirements(program, &mut diagnostics);
    validate_data_conformances(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    properties::validate_data_properties(program, &symbols, &mut diagnostics);
    // Bare-payload-case `==` (decision 11) is checked on the RESOLVED trees,
    // before membership lowering synthesizes its internal tag compares; see
    // omega-symbol-resolved-trees-to-typed-trees/src/equality.rs.
    struct_literals::validate_struct_literal_fields(program, &mut diagnostics);
    wire::validate_wire_schemas(program, &symbols, &mut diagnostics);
    operators::validate_operator_declarations(program, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        validate_contained_types(program, machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);
        validate_version_scoped_target(program, machine, &mut diagnostics);
        validate_machine_effects(program, machine, &mut diagnostics);
        validate_machine_contracts(program, machine, &mut diagnostics);
        validate_machine_contract_entailment(program, machine, &mut diagnostics);
        validate_machine_trait_conformances(program, machine, &mut diagnostics);

        for state in program.machine_states(machine) {
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            let writable_roots = WritableRoots {
                machine_symbols: &machine_symbols,
                statements: program.statement_table.statements(state.statement_nodes),
                parameters: program.state_parameters(state),
            };

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement,
                    &mut diagnostics,
                );
            }

            // VALUE-position calls inside expression trees (LocalData
            // initializers, transition arguments, guard subjects, etc.)
            // are not reached by `validate_state_statement_node`; run the
            // bound check for them separately.
            validate_value_position_calls(
                program,
                machine,
                state,
                &machine_symbols,
                &symbols,
                &writable_roots,
                &mut diagnostics,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => validate_assignment_target_handle(
            program,
            assignment.target,
            writable_roots,
            diagnostics,
            machine.name.as_str(),
            state_name,
        ),
        StatementNode::Call(call) => validate_call_node(
            program,
            call,
            machine,
            state_name,
            machine_symbols,
            symbols,
            writable_roots,
            diagnostics,
        ),
        StatementNode::Expression(expression) => {
            let Some(state) = machine_symbols.state(state_name) else {
                return;
            };

            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
        }
        StatementNode::LocalData(local_data) => validate_type_reference_handle(
            program,
            local_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::StateLocalData {
                machine: machine.name.as_str(),
                state: state_name,
                local: local_data.name.as_str(),
                generic_depth: 0,
            },
        ),
        StatementNode::Transition(transition) => {
            validate_transition_target_node(
                program,
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }
        }
    }
}
