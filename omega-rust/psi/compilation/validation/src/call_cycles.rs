/*
Enumerate source declarations used by a machine or contract. These queries
collect identities; lowered checking owns recursion and ranking judgments.
*/
use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub fn machine_call_dependency_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Vec<SymbolHandle> {
    let mut symbols = Vec::new();
    collect_contract_dependency_symbols(program, program.machine_contracts(machine), &mut symbols);
    for state in program.machine_states(machine) {
        collect_contract_dependency_symbols(program, program.state_contracts(state), &mut symbols);
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_dependency_symbols(program, statement, &mut symbols);
        }
    }
    symbols.sort_unstable_by_key(|symbol| symbol.arena_index());
    symbols.dedup();
    symbols
}

pub(crate) fn declaration_call_dependencies(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    if program
        .symbols
        .builtin_function_for_symbol(symbol)
        .is_some()
        || symbols::FixedCoreDeclaration::ALL
            .into_iter()
            .any(|declaration| {
                program.symbols.fixed_core_declaration_symbol(declaration) == Some(symbol)
            })
    {
        return Some((symbol, Vec::new()));
    }
    let machine = program.machine_by_symbol(symbol).or_else(|| {
        crate::machine_calls::calls::machine_state_by_symbol(program, symbol)
            .map(|(machine, _)| machine)
    });
    if let Some(machine) = machine {
        let mut dependencies = machine_call_dependency_symbols(program, machine);
        dependencies.extend(
            crate::machine_calls::machine_parameters::specialization_selection_dependency_symbols(
                program,
                machine.symbol,
            )?,
        );
        if machine.target_family_interface {
            dependencies.extend(
                crate::machine_calls::calls::target_family_members(program, machine)?
                    .iter()
                    .map(|member| member.machine),
            );
        }
        return Some((machine.symbol, dependencies));
    }
    let mut dependencies = Vec::new();
    if let Some(requirement) = program
        .dynamic_slot_by_symbol(symbol)
        .and_then(|slot| program.dynamic_slot_requirement(slot))
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(requirement),
            &mut dependencies,
        );
    } else if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    {
        for root in crate::mathematical_applications::definition_roots(program, definition) {
            collect_expression_dependency_symbols(program, root, &mut dependencies);
        }
    } else if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) {
        collect_contract_dependency_symbols(
            program,
            program.operator_contracts(operator),
            &mut dependencies,
        );
    } else if let Some(signature) = crate::selected_requirement_signature(program, symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program
                .machine_parameter_signature(symbol)
                .map(|(_, signature)| signature)
        })
        .or_else(|| {
            program
                .retained_static_machine_contract(symbol)
                .map(|contract| contract.signature())
        })
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(signature),
            &mut dependencies,
        );
        dependencies.extend(requirement_backing_machines(program, symbol));
    } else {
        let proposition = program
            .propositions()
            .iter()
            .find(|proposition| proposition.symbol == symbol)?;
        if let typed_trees::proposition::PropositionBody::Transparent { proposition } =
            &proposition.body
        {
            match proposition {
                typed_trees::proposition::PropositionFormula::BooleanExpression(expression) => {
                    collect_expression_dependency_symbols(program, *expression, &mut dependencies);
                }
                typed_trees::proposition::PropositionFormula::Application(application) => {
                    collect_proposition_dependency_symbols(program, application, &mut dependencies);
                }
            }
        }
    }
    dependencies
        .sort_unstable_by_key(|dependency| (dependency.arena_index(), dependency.generation()));
    dependencies.dedup();
    Some((symbol, dependencies))
}

pub fn requirement_backing_machines(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Vec<SymbolHandle> {
    let declared = program
        .closed_requirement_application(symbol)
        .map_or(symbol, |application| application.template);
    if program.trait_machine_signature(declared).is_none() {
        return Vec::new();
    }
    let mut backing = typed_trees::trait_definition::declared_requirement_realizers(
        program.typed_program(),
        declared,
    );
    backing.extend(
        program
            .conformances()
            .iter()
            .flat_map(|conformance| conformance.rows.iter())
            .filter(|row| row.requirement == declared && row.realization_machine.is_valid())
            .map(|row| row.realization_machine),
    );
    backing
}

fn collect_contract_dependency_symbols(
    program: &dyn crate::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    symbols: &mut Vec<SymbolHandle>,
) {
    for fact in contracts
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
    {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                collect_expression_dependency_symbols(program, *expression, symbols);
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                collect_expression_dependency_symbols(program, membership.value, symbols);
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                collect_proposition_dependency_symbols(program, application, symbols);
            }
        }
    }
}

fn collect_proposition_dependency_symbols(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    symbols: &mut Vec<SymbolHandle>,
) {
    push_dependency_symbol(symbols, application.proposition);
    for argument in expression_table(program).expression_handles(application.arguments) {
        collect_expression_dependency_symbols(program, *argument, symbols);
    }
    for argument in &application.binder_arguments {
        if argument.kind == typed_trees::proposition::PropositionBinderArgumentKind::Machine {
            push_dependency_symbol(symbols, argument.symbol);
        }
    }
}

fn push_dependency_symbol(symbols: &mut Vec<SymbolHandle>, symbol: SymbolHandle) {
    if symbol.is_valid() {
        symbols.push(symbol);
    }
}

fn collect_statement_dependency_symbols(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    symbols: &mut Vec<SymbolHandle>,
) {
    match statement {
        StatementNode::Assembly(_) => {}
        StatementNode::AssemblyFact(fact) => {
            for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                collect_expression_dependency_symbols(program, expression, symbols);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_dependency_symbols(program, assignment.target, symbols);
            collect_expression_dependency_symbols(program, assignment.value, symbols);
        }
        StatementNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            symbols.extend(
                crate::machine_calls::machine_parameters::call_selection_dependency_symbols(
                    program,
                    typed_trees::references::StaticArgumentCall::Statement(call),
                ),
            );
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_dependency_symbols(program, *argument, symbols);
            }
        }
        StatementNode::Expression(handle) => {
            collect_expression_dependency_symbols(program, *handle, symbols)
        }
        StatementNode::LocalData(local) => {
            collect_expression_dependency_symbols(program, local.initial_value, symbols)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_dependency_symbols(program, guard, symbols);
            }
            for target_handle in transition.targets() {
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        push_dependency_symbol(symbols, path.symbol);
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression_dependency_symbols(program, *argument, symbols);
                        }
                    }
                    TransitionTargetNode::Value(handle) => {
                        collect_expression_dependency_symbols(program, *handle, symbols)
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_dependency_symbols(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    let node = expression_table(program).expression(expression);
    if let ExpressionNode::Call(call) = node {
        push_dependency_symbol(symbols, call.target_symbol);
        symbols.extend(
            crate::machine_calls::machine_parameters::call_selection_dependency_symbols(
                program,
                typed_trees::references::StaticArgumentCall::Expression(expression, call),
            ),
        );
    }
    crate::queries::expression_traversal::executable_children(program, node, |child| {
        collect_expression_dependency_symbols(program, child, symbols);
    });
}
