//! Generic-data method clones retain their template's application requirements.
//!
//! Early container specialization clears the clone's binder list. Its exact
//! template and closed owner still identify the arguments, so check selected
//! calls against that original contract rather than mistaking the clone for an
//! unconstrained declaration. Requirements belong to method use, not to every
//! instance of its container; an unused constrained method grants no call.

use std::collections::HashSet;

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{TypedTrees, expression::ExpressionNode, statement::StatementNode};

pub(crate) fn validate_selected_attached_method_bounds(
    program: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    if !program
        .machines()
        .iter()
        .any(|machine| machine.generic_data_template.is_valid())
    {
        return Ok(());
    }
    let mut selected = HashSet::<SymbolHandle>::new();
    let mut expressions = Vec::new();
    for constant in program.const_declarations() {
        super::collect_expression_tree(program, constant.authored_initializer, &mut expressions);
        super::collect_expression_tree(
            program,
            constant.materialized_initializer,
            &mut expressions,
        );
    }
    for call in program
        .proof_output_calls
        .iter()
        .filter(|call| !call.machine_symbol.is_valid())
    {
        super::collect_expression_tree(program, call.call, &mut expressions);
    }
    collect_expression_selections(program, &expressions, &mut selected);
    // Container normalization eagerly generates every method. Only authored
    // bodies and demanded generated bodies impose concrete call requirements;
    // visiting an unused clone would strengthen the container declaration.
    let mut visited = vec![false; program.machines().len()];
    loop {
        let mut advanced = false;
        for (machine, visited) in program.machines().iter().zip(&mut visited) {
            if *visited
                || (machine.generic_data_template.is_valid()
                    && !method_is_selected(program, machine, &selected))
            {
                continue;
            }
            *visited = true;
            advanced = true;
            expressions.clear();
            for call in program
                .proof_output_calls
                .iter()
                .filter(|call| call.machine_symbol == machine.symbol)
            {
                super::collect_expression_tree(program, call.call, &mut expressions);
            }
            for contract in program.machine_contracts(machine) {
                crate::monomorphization::selection::collect_contract_facts(
                    program,
                    contract.facts,
                    &mut expressions,
                );
            }
            for owned in program.machine_owned_data(machine) {
                super::collect_expression_tree(program, owned.initial_value, &mut expressions);
            }
            for state in program.machine_states(machine) {
                for contract in program.state_contracts(state) {
                    crate::monomorphization::selection::collect_contract_facts(
                        program,
                        contract.facts,
                        &mut expressions,
                    );
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    super::collect_statement_expression_trees(program, statement, &mut expressions);
                    if let StatementNode::Call(call) = statement {
                        selected.insert(call.target_symbol);
                        collect_static_selections(&call.machine_arguments, &mut selected);
                    }
                }
            }
            collect_expression_selections(program, &expressions, &mut selected);
        }
        if !advanced {
            break;
        }
    }
    let mut diagnostics = Vec::new();
    let symbols = validation::TopLevelSymbols::build(program, &mut diagnostics);
    for machine in program.machines().iter().filter(|machine| {
        machine.generic_data_template.is_valid() && method_is_selected(program, machine, &selected)
    }) {
        let Some(template) =
            crate::lookup::machine_by_symbol(program, machine.generic_data_template)
        else {
            diagnostics.push(Diagnostic::error(
                "generic-data method lost its exact template",
            ));
            continue;
        };
        let application = program
            .type_reference_table
            .type_reference(template.attached_data_application);
        let concrete = program
            .data_definitions()
            .iter()
            .find(|owner| owner.symbol == machine.attached_data_symbol)
            .and_then(|owner| owner.generic_instance)
            .map(|application| program.type_reference_table.type_reference(application));
        let (
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            },
            Some(TypeReferenceNode::Generic {
                base_symbol: concrete_base,
                arguments: concrete_arguments,
                ..
            }),
        ) = (application, concrete)
        else {
            diagnostics.push(Diagnostic::error(
                "generic-data method lost its exact owner application",
            ));
            continue;
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let concrete_arguments = program
            .type_reference_table
            .type_reference_handles(*concrete_arguments);
        if *base_symbol != template.attached_data_symbol
            || base_symbol != concrete_base
            || arguments.len() != concrete_arguments.len()
        {
            diagnostics.push(Diagnostic::error(
                "generic-data method owner application does not match its template",
            ));
            continue;
        }
        for parameter in program.machine_type_parameters(template) {
            let requirements = validation::declared_property_requirements(&parameter.bounds);
            if requirements.is_empty() {
                continue;
            }
            let argument = arguments
                .iter()
                .zip(concrete_arguments)
                .find_map(|(formal, actual)| {
                    matches!(program.type_reference_table.type_reference(*formal),
                    TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
                    .then_some(*actual)
                });
            if !argument.is_some_and(|argument| {
                requirements.iter().all(|requirement| {
                    validation::type_satisfies_declared_property(
                        program,
                        &symbols,
                        &[],
                        argument,
                        *requirement,
                    )
                })
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "generic method `{}` has a selected owner argument that does not satisfy its authored type bounds for `{}`",
                    template.name, parameter.name,
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn method_is_selected(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    selected: &HashSet<SymbolHandle>,
) -> bool {
    selected.contains(&machine.symbol)
        || program
            .machine_states(machine)
            .iter()
            .any(|state| selected.contains(&state.symbol))
}

fn collect_expression_selections(
    program: &TypedTrees,
    expressions: &[typed_trees::expression::ExpressionHandle],
    selected: &mut HashSet<SymbolHandle>,
) {
    for expression in expressions {
        if let ExpressionNode::Call(call) = program.expression_table.expression(*expression) {
            selected.insert(call.target_symbol);
            collect_static_selections(&call.machine_arguments, selected);
        }
    }
}

fn collect_static_selections(
    arguments: &[typed_trees::expression::StaticMachineArgument],
    selected: &mut HashSet<SymbolHandle>,
) {
    for argument in arguments {
        selected.insert(argument.symbol);
        if let Some(application) = &argument.application {
            collect_static_selections(&application.arguments, selected);
        }
    }
}
