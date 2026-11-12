/*
    Symbol assignment for statement bodies, state signatures and lexical inputs.
    `routing` stamps each statement so it sees only its predecessors, and
    `assembly` scopes inline assembly temporaries to their enclosing prefix.
*/

mod assembly {
    /*
        Assembly instructions see their enclosing lexical prefix and their own
        preceding temporaries. A sibling label never supplies a lexical binding.
    */

    use super::routing;
    use crate::symbols::expressions::assign_statement_expression_symbols;
    use crate::symbols::scope::MachineScope;
    use arena::{Arena, HandleSpan};
    use symbol_resolved_trees::{expression, name, signature, statement, types};
    use symbols::{SymbolHandle, SymbolTable};

    pub(super) struct AssemblyScopeRouting<'scope> {
        pub graphs: &'scope Arena<statement::AssemblyControlFlow>,
        pub blocks: &'scope Arena<statement::AssemblyBlock>,
        pub contracts: &'scope Arena<expression::ExpressionHandle>,
        pub statements: &'scope mut Arena<statement::Statement>,
        pub expressions: &'scope mut expression::ExpressionTable,
        pub child_types: &'scope mut Arena<types::TypeReference>,
        pub type_constraints: &'scope Arena<types::TypeConstraint>,
        pub path_members: &'scope mut Arena<name::DiagnosticName>,
        pub symbols: &'scope SymbolTable,
    }

    impl AssemblyScopeRouting<'_> {
        pub(super) fn assign(
            &mut self,
            machine: &MachineScope<'_>,
            parameters: &[signature::StateParameter],
            state: SymbolHandle,
            outer: HandleSpan<statement::Statement>,
        ) {
            let outer = self.statements.span_or_empty(outer).to_vec();
            for (index, statement) in outer.iter().enumerate() {
                let statement::Statement::Assembly(handle) = statement else {
                    continue;
                };
                let graph = self.graphs.get(*handle);
                let entry = MachineScope {
                    prior_statements: &outer[..index],
                    ..*machine
                };
                for expression in self.contracts.span_or_empty(graph.primitives) {
                    assign_statement_expression_symbols(
                        self.symbols,
                        &entry,
                        parameters,
                        state,
                        self.expressions,
                        self.child_types,
                        *expression,
                    );
                }
                for expression in graph
                    .scratch
                    .iter()
                    .copied()
                    .filter(|expression| expression.is_valid())
                {
                    assign_statement_expression_symbols(
                        self.symbols,
                        &entry,
                        parameters,
                        state,
                        self.expressions,
                        self.child_types,
                        expression,
                    );
                }
                for block in self.blocks.span_or_empty(graph.blocks) {
                    let prefix = &outer[..index];
                    let entry = MachineScope {
                        prior_statements: prefix,
                        ..*machine
                    };
                    for expression in self.contracts.span_or_empty(block.requires) {
                        assign_statement_expression_symbols(
                            self.symbols,
                            &entry,
                            parameters,
                            state,
                            self.expressions,
                            self.child_types,
                            *expression,
                        );
                    }
                    let mut sequence = prefix.to_vec();
                    sequence.extend_from_slice(self.statements.span_or_empty(block.instructions));
                    for index in prefix.len()..sequence.len() {
                        let (prior, next) = sequence.split_at_mut(index);
                        let scope = MachineScope {
                            prior_statements: prior,
                            ..*machine
                        };
                        routing::assign_statement_symbols(
                            &scope,
                            parameters,
                            state,
                            self.expressions,
                            self.child_types,
                            self.type_constraints,
                            self.path_members,
                            &mut next[0],
                            self.symbols,
                        );
                    }
                    let exit = MachineScope {
                        prior_statements: &sequence,
                        ..*machine
                    };
                    for expression in self.contracts.span_or_empty(block.ensures) {
                        assign_statement_expression_symbols(
                            self.symbols,
                            &exit,
                            parameters,
                            state,
                            self.expressions,
                            self.child_types,
                            *expression,
                        );
                    }
                    self.statements
                        .span_mut_or_empty(block.instructions)
                        .clone_from_slice(&sequence[prefix.len()..]);
                }
            }
        }
    }
}
mod routing {
    use arena::Arena;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::expressions::{
        assign_expression_span_symbols, assign_statement_expression_symbols,
    };
    use crate::symbols::scope::MachineScope;
    use crate::symbols::scoped_paths::resolve_state_scoped_members;
    use crate::symbols::targets::{
        assign_call_machine_argument_symbols, assign_transition_target_symbols,
        resolve_call_target_symbol,
    };
    use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;
    use symbol_resolved_trees::statement::TransitionGuard;

    pub(super) fn assign_local_type_symbols(
        machine: &MachineScope<'_>,
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        reference: &mut symbol_resolved_trees::types::TypeReference,
        symbols: &SymbolTable,
    ) {
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            machine.type_parameters,
            reference,
        );
        crate::symbols::type_references::assign_type_value_expression_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            reference,
        );
    }

    /*
        Assigns symbols across a statement slice so each statement sees only its
        stamped predecessors: receivers and locals may name earlier statements,
        never their own binding or a later one.
    */
    pub(super) fn assign_statement_slice_symbols(
        machine_scope: &MachineScope<'_>,
        statements: &mut [symbol_resolved_trees::statement::Statement],
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &arena::Arena<symbol_resolved_trees::types::TypeConstraint>,
        statement_path_members: &mut Arena<symbol_resolved_trees::name::DiagnosticName>,
        symbols: &SymbolTable,
    ) {
        for index in 0..statements.len() {
            let (prior_statements, remaining) = statements.split_at_mut(index);
            let scope = MachineScope {
                prior_statements,
                ..*machine_scope
            };
            assign_statement_symbols(
                &scope,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                type_constraints,
                statement_path_members,
                &mut remaining[0],
                symbols,
            );
        }
    }

    pub(super) fn assign_statement_symbols(
        machine: &MachineScope<'_>,
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &arena::Arena<symbol_resolved_trees::types::TypeConstraint>,
        statement_path_members: &mut Arena<symbol_resolved_trees::name::DiagnosticName>,
        statement: &mut symbol_resolved_trees::statement::Statement,
        symbols: &SymbolTable,
    ) {
        macro_rules! assign_expr {
            ($expression:expr) => {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    $expression,
                )
            };
        }

        match statement {
            symbol_resolved_trees::statement::Statement::Assembly(_) => {}
            symbol_resolved_trees::statement::Statement::RootBinding(binding) => {
                assign_expr!(binding.receiver);
                if binding.implementation_operand.is_valid() {
                    assign_expr!(binding.implementation_operand);
                    let described = match expression_table
                        .expression(binding.implementation_operand)
                    {
                        symbol_resolved_trees::expression::ExpressionNode::Name(path) => matches!(
                            symbols.get(path.symbol).kind,
                            SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                        ),
                        _ => true,
                    };
                    if !described {
                        binding.implementation_operand =
                            symbol_resolved_trees::expression::ExpressionHandle::invalid();
                    }
                }
            }
            symbol_resolved_trees::statement::Statement::AssemblyFact(fact) => {
                assign_expr!(fact.expression);
            }
            symbol_resolved_trees::statement::Statement::Assignment(assignment) => {
                assign_expr!(assignment.target);
                assign_expr!(assignment.value);
            }
            symbol_resolved_trees::statement::Statement::Call(call) => {
                if call.callee.is_valid() {
                    assign_expr!(call.callee);
                }
                assign_expression_span_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    call.arguments,
                );
                if !call.receiver.is_empty() {
                    let members = statement_path_members.span_or_empty(call.receiver);
                    let (root, _) = resolve_state_scoped_members(
                        symbols,
                        machine.symbol,
                        state_symbol,
                        members.get(..1).unwrap_or_default(),
                        None,
                    );
                    call.receiver_root_symbol = root;
                    let (_, symbol) = resolve_state_scoped_members(
                        symbols,
                        machine.symbol,
                        state_symbol,
                        members,
                        None,
                    );
                    if symbol.is_valid() {
                        call.receiver_symbol = symbol;
                    }
                }

                call.target_symbol = resolve_call_target_symbol(
                    machine,
                    parameters,
                    !call.receiver.is_empty(),
                    call.receiver_symbol,
                    &call.target,
                    child_type_references,
                    symbols,
                );
                crate::symbols::expressions::assign_static_arguments_type_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    &call.machine_arguments,
                );
                assign_call_machine_argument_symbols(
                    symbols,
                    machine,
                    state_symbol,
                    &call.target,
                    call.target_symbol,
                    &mut call.storage.machine_arguments,
                );
            }
            symbol_resolved_trees::statement::Statement::ProofOutputBindingStatement(binding) => {
                binding.machine_symbol = machine.symbol;
                binding.state_symbol = state_symbol;
                assign_expr!(binding.call);
            }
            symbol_resolved_trees::statement::Statement::Expression(expression) => {
                assign_expr!(*expression);
            }
            symbol_resolved_trees::statement::Statement::LocalData(local_data) => {
                assign_local_type_symbols(
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_constraints,
                    &mut local_data.type_reference,
                    symbols,
                );
                if local_data.initial_value.is_valid() {
                    assign_expr!(local_data.initial_value);
                }
            }
            symbol_resolved_trees::statement::Statement::Transition(transition) => {
                use symbol_resolved_trees::expression::{
                    BinaryOperator, ExpressionNode, TableBinaryExpression,
                    TableMembershipExpression,
                };
                let original_guard = transition
                    .source_guard
                    .clone()
                    .unwrap_or_else(|| transition.guard.clone());
                if !transition.patterns.is_empty() {
                    transition.source_guard = Some(original_guard.clone());
                }
                let mut selected_guard = None;
                for retained in &transition.patterns {
                    if retained.pattern.application_type.is_valid() {
                        crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                            symbols,
                            machine,
                            parameters,
                            state_symbol,
                            expression_table,
                            child_type_references,
                            retained.pattern.application_type,
                        );
                    }
                    assign_expr!(retained.subject);
                    crate::symbols::expressions::assign_pattern_head_symbols(
                        symbols,
                        machine,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        retained.pattern.head,
                    );
                    let head = expression_table
                        .name_path_node(retained.pattern.head)
                        .cloned();
                    let selected = head.as_ref().map(|head| head.symbol).unwrap_or_default();
                    for position in 0..retained.pattern.fields.count() {
                        let handle = retained.pattern.fields.handle_at(position);
                        let field = &expression_table.match_fields(retained.pattern.fields)
                            [position as usize];
                        let symbol = crate::symbols::expression_paths::pattern_field_symbol(
                            symbols,
                            selected,
                            field.member.as_str(),
                        );
                        expression_table.match_field_mut(handle).field_symbol = symbol;
                    }
                    assign_expr!(retained.pattern.condition);
                    let test = if selected.is_valid()
                        && matches!(
                            symbols.get(selected).kind,
                            SymbolKind::Variant | SymbolKind::Domain
                        ) {
                        let head = head.as_ref().expect("selected pattern head");
                        let is_case = symbols.get(selected).kind == SymbolKind::Variant;
                        Some(expression_table.insert(ExpressionNode::Membership(
                            TableMembershipExpression {
                                is_pattern: true,
                                value: retained.subject,
                                domain: head.members,
                                domain_symbol: if is_case {
                                    SymbolHandle::invalid()
                                } else {
                                    selected
                                },
                                case_type_symbol: if is_case {
                                    symbols.get(selected).parent
                                } else {
                                    SymbolHandle::invalid()
                                },
                                case_symbol: if is_case {
                                    selected
                                } else {
                                    SymbolHandle::invalid()
                                },
                            },
                        )))
                    } else if !retained.is_structural && retained.pattern.head.is_valid() {
                        Some(expression_table.insert(ExpressionNode::Binary(
                            TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left: retained.subject,
                                operator: BinaryOperator::Equal,
                                right: retained.pattern.head,
                            },
                        )))
                    } else {
                        None
                    };
                    let axis = if let Some(test) = test {
                        if matches!(
                            expression_table.expression(retained.pattern.condition),
                            ExpressionNode::Boolean(true)
                        ) {
                            test
                        } else {
                            expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left: test,
                                operator: BinaryOperator::And,
                                right: retained.pattern.condition,
                            }))
                        }
                    } else {
                        retained.pattern.condition
                    };
                    selected_guard = Some(if let Some(prior) = selected_guard {
                        expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: prior,
                            operator: BinaryOperator::And,
                            right: axis,
                        }))
                    } else {
                        axis
                    });
                }
                if let Some(test) = selected_guard {
                    transition.guard = TransitionGuard::When(match original_guard {
                        TransitionGuard::Always => test,
                        TransitionGuard::When(predicate) => {
                            expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left: test,
                                operator: BinaryOperator::And,
                                right: predicate,
                            }))
                        }
                    });
                }

                if let TransitionGuard::When(expression) = &mut transition.guard {
                    assign_expr!(*expression);
                }
                assign_transition_target_symbols(
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    &mut transition.target,
                    symbols,
                );
                if let Some(continuation) = &mut transition.continuation {
                    assign_transition_target_symbols(
                        machine,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        statement_path_members,
                        continuation,
                        symbols,
                    );
                }
            }
        }
    }
}

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

/*
    Signature value expressions and ranking declarations are needed before any
    implementation is selected; no statement or local enters this traversal.
*/
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
    let conformance_scopes = super::scope::DeclarationTypeScopes::of(program)?;
    let SymbolResolvedTrees {
        roots:
            symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                type_aliases,
                conformances,
                traits,
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
    let assembly_graphs = &tables.declarations.assembly_graphs;
    let assembly_blocks = &tables.declarations.assembly_blocks;
    let assembly_expressions = &tables.declarations.assembly_expressions;
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
            lexical_requirements: trait_scopes
                .requirements_of(machine.trait_default_origin.declaring_trait),
            type_parameters: &lexical_type_parameters,
            attached_data_symbol: machine.attached_data_symbol,
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
            prior_statements: &[],
            data_definitions,
            type_aliases,
            conformances,
            traits,
            operand_owners: &tables.types.operand_lifetime_owners,
            data_members,
            data_type_parameters,
            data_payload_fields: &tables.declarations.data_payload_fields,
            type_constraints,
        };
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
                for (preceding, expression) in &inputs.calls {
                    let scope = MachineScope {
                        prior_statements: &inputs.locals[..*preceding],
                        ..machine_scope
                    };
                    super::expressions::assign_expression_table_symbols(
                        symbols,
                        &scope,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        *expression,
                    );
                }
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
            assembly::AssemblyScopeRouting {
                graphs: assembly_graphs,
                blocks: assembly_blocks,
                contracts: assembly_expressions,
                statements: state_statements,
                expressions: expression_table,
                child_types: child_type_references,
                type_constraints,
                path_members: statement_path_members,
                symbols,
            }
            .assign(&machine_scope, parameters, state_symbol, state.statements);
        }
    });
    Ok(())
}
