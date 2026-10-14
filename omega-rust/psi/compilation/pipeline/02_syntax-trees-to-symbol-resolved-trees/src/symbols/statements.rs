mod routing;
mod tail_calls;

use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolTable;

use super::expressions::{assign_expression_span_symbols, assign_statement_expression_symbols};
use super::scope::MachineScope;

pub(super) fn assign_statement_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    assign_machine_references(program, symbols, Some(selected_machines), &mut [])
}

/// Signature value expressions and ranking declarations are needed before any
/// implementation is selected; no statement or local enters this traversal.
pub(super) fn assign_machine_signature_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    assign_machine_references(program, symbols, None, &mut [])
}

pub(super) fn assign_lexical_input_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    inputs: &mut [crate::lowering::state::StateLexicalInputs],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    assign_machine_references(program, symbols, None, inputs)
}

fn assign_machine_references(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    selected_bodies: Option<&[symbols::SymbolHandle]>,
    lexical_inputs: &mut [crate::lowering::state::StateLexicalInputs],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let attached_machines = super::scope::attached_machines(program);
    let trait_scopes = super::scope::TraitLexicalScopes::of(program);
    let conformance_scopes = super::scope::ConformanceTypeScopes::of(program)?;
    let SymbolResolvedTrees {
        roots:
            symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let data_type_parameters = &tables.declarations.data_type_parameters;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &mut tables.declarations.machine_states;
    let state_parameters = &mut tables.declarations.state_parameters;
    let statement_path_members = &mut tables.declarations.statement_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let state_statements = &mut tables.declarations.state_statements;
    let child_type_references = &mut tables.declarations.child_type_references;
    let type_constraints = &tables.types.constraints;
    machines.for_each_mut(|machine| {
        if selected_bodies.is_some_and(|selected| !selected.contains(&machine.symbol)) {
            return;
        }
        let machine_symbol = machine.symbol;
        let machine_type_parameters = machine.type_parameters;
        let data_definition = data_definitions
            .iter()
            .find(|data_definition| data_definition.symbol == machine.attached_data_symbol);
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let invocation_scope = super::scope::declaration_type_scope(
            data_type_parameters.span_or_empty(machine_type_parameters),
            conformance_scopes.parameters(machine_symbol),
        );
        let lexical_type_parameters = super::scope::declaration_type_scope(
            &invocation_scope,
            data_definition.map_or(&[], |owner| {
                data_type_parameters.span_or_empty(owner.type_parameters)
            }),
        );
        let symbol_resolved_trees::machine::MachineStorage {
            owned_data,
            satisfies: _,
            ranking_subjects,
            ranking_view_arguments,
            ranking_range,
            ranking_view: _,
            contracts: _,
            states,
            ..
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            symbol: machine_symbol,
            // An instantiated trait default body keeps its authoring trait's
            // requirement names; every other machine is outside any trait.
            lexical_requirements: trait_scopes
                .requirements_of(machine.trait_default_origin.declaring_trait),
            type_parameters: &lexical_type_parameters,
            attached_data: machine.attached_data.as_ref(),
            attached_data_symbol: machine.attached_data_symbol,
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
            prior_statements: &[],
            data_definitions,
            data_members,
            data_type_parameters,
            data_payload_fields: &tables.declarations.data_payload_fields,
            type_constraints,
        };
        // Ranking expressions belong to the entry signature, not a later
        // statement or a same-spelled parameter in another state. Resolve
        // their retained handles before proof/termination consumers use them.
        if let Some(entry) = machine_state_handles.span_or_empty(*states).first() {
            let entry = machine_states.get(*entry);
            let parameters = state_parameters.span_or_empty(entry.parameters);
            for expressions in [*ranking_subjects, *ranking_view_arguments] {
                assign_expression_span_symbols(
                    symbols,
                    &machine_scope,
                    parameters,
                    entry.symbol,
                    expression_table,
                    child_type_references,
                    expressions,
                );
            }
            assign_statement_expression_symbols(
                symbols,
                &machine_scope,
                parameters,
                entry.symbol,
                expression_table,
                child_type_references,
                *ranking_range,
            );
        }
        let entry = machine_state_handles
            .span_or_empty(*states)
            .first()
            .map(|entry| {
                let entry = machine_states.get(*entry);
                (entry.symbol, entry.return_type.clone())
            });
        for state_handle in machine_state_handles.span_or_empty(*states).iter().copied() {
            let state = machine_states.get_mut(state_handle);
            let state_symbol = state.symbol;
            let parameter_frontier = state_parameters.span_or_empty(state.parameters).to_vec();
            let parameters = parameter_frontier.as_slice();
            for (offset, parameter) in parameters.iter().enumerate() {
                let mut type_reference = parameter.type_reference.clone();
                super::type_references::assign_type_value_expression_symbols(
                    symbols,
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    &mut type_reference,
                );
                let handle = state.parameters.handle_at(offset as u32);
                state_parameters.get_mut(handle).type_reference = type_reference;
            }
            if let Some(type_reference) = &mut state.return_type {
                super::type_references::assign_type_value_expression_symbols(
                    symbols,
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_reference,
                );
            }
            if let Some(inputs) = lexical_inputs
                .iter_mut()
                .find(|inputs| inputs.machine == machine_symbol && inputs.state == state_handle)
            {
                routing::assign_statement_slice_symbols(
                    &machine_scope,
                    &mut inputs.locals,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_constraints,
                    statement_path_members,
                    symbols,
                );
                for (preceding, reference) in &mut inputs.types {
                    let scope = MachineScope {
                        prior_statements: &inputs.locals[..*preceding],
                        ..machine_scope
                    };
                    routing::assign_local_type_symbols(
                        &scope,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        type_constraints,
                        reference,
                        symbols,
                    );
                }
            }
            if selected_bodies.is_none() {
                continue;
            }
            let statements = state_statements.span_mut_or_empty(state.statements);
            routing::assign_statement_slice_symbols(
                &machine_scope,
                statements,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                type_constraints,
                statement_path_members,
                symbols,
            );
            // A value call checks its result against this state's complete
            // destination type. A jump has no value-result check, so only
            // normalize when the full resolved result declarations agree.
            // Child handles/constraints remain part of equality: uncertain
            // equivalence keeps the original call for ordinary checking.
            if let Some((entry_symbol, ref entry_return_type)) = entry
                && state.return_type == *entry_return_type
            {
                tail_calls::normalize_recursive_tail_calls(
                    &machine_scope,
                    symbols,
                    entry_symbol,
                    expression_table,
                    statement_path_members,
                    statements,
                );
            }
        }
    });
    Ok(())
}
