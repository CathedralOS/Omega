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
    use crate::resolution::names::expressions::assign_statement_expression_symbols;
    use crate::resolution::names::scopes::MachineScope;
    use crate::source_resolved::{expression, name, signature, statement, types};
    use crate::source_symbols::SymbolTable;
    use arena::{Arena, HandleSpan};
    use symbols::SymbolHandle;

    pub(super) struct AssemblyScopeRouting<'scope> {
        pub graphs: &'scope Arena<statement::AssemblyControlFlow>,
        pub blocks: &'scope Arena<statement::AssemblyBlock>,
        pub contracts: &'scope Arena<expression::ExpressionHandle>,
        pub statements: &'scope mut Arena<statement::Statement>,
        pub expressions: &'scope mut expression::ExpressionTable,
        pub child_types: &'scope mut crate::source_resolved::types::TypeReferenceArena,
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
mod blocks {
    /*
        A block value's statements see the statements before the statement
        that holds the block, then the block's own earlier statements; its
        final value sees all of them. The block's locals are absent from every
        other sequence, so they leave scope at its end. The enclosing
        statement's own walk leaves the block to this routing.
    */

    use super::routing;
    use crate::resolution::names::expressions::assign_statement_expression_symbols;
    use crate::resolution::names::scopes::MachineScope;
    use crate::source_resolved::{expression, name, signature, statement, types};
    use crate::source_symbols::SymbolTable;
    use arena::{Arena, HandleSpan};
    use symbols::SymbolHandle;

    pub(super) struct BlockScopeRouting<'scope> {
        pub statements: &'scope mut Arena<statement::Statement>,
        pub expressions: &'scope mut expression::ExpressionTable,
        pub child_types: &'scope mut crate::source_resolved::types::TypeReferenceArena,
        pub type_constraints: &'scope Arena<types::TypeConstraint>,
        pub path_members: &'scope mut Arena<name::DiagnosticName>,
        pub symbols: &'scope SymbolTable,
    }

    impl BlockScopeRouting<'_> {
        pub(super) fn assign(
            &mut self,
            machine: &MachineScope<'_>,
            parameters: &[signature::StateParameter],
            state: SymbolHandle,
            outer: HandleSpan<statement::Statement>,
        ) {
            let outer = self.statements.span_or_empty(outer).to_vec();
            self.sequence(machine, parameters, state, &outer, 0);
        }

        /// The blocks of `sequence[first..]`, each seeing the statements
        /// before the statement that holds it.
        fn sequence(
            &mut self,
            machine: &MachineScope<'_>,
            parameters: &[signature::StateParameter],
            state: SymbolHandle,
            sequence: &[statement::Statement],
            first: usize,
        ) {
            for index in first..sequence.len() {
                let mut operands = Vec::new();
                sequence[index].visit_operands(self.expressions, |operand| operands.push(operand));
                for operand in operands {
                    self.blocks_in(machine, parameters, state, &sequence[..index], operand);
                }
            }
        }

        fn blocks_in(
            &mut self,
            machine: &MachineScope<'_>,
            parameters: &[signature::StateParameter],
            state: SymbolHandle,
            prefix: &[statement::Statement],
            expression: expression::ExpressionHandle,
        ) {
            let mut blocks = Vec::new();
            self.expressions.direct_blocks(expression, &mut blocks);
            for block in blocks {
                self.block(machine, parameters, state, prefix, block);
            }
        }

        fn block(
            &mut self,
            machine: &MachineScope<'_>,
            parameters: &[signature::StateParameter],
            state: SymbolHandle,
            prefix: &[statement::Statement],
            block: expression::ExpressionHandle,
        ) {
            let expression::ExpressionNode::Block(found) = *self.expressions.expression(block)
            else {
                return;
            };
            let mut sequence = prefix.to_vec();
            let start = sequence.len();
            sequence.extend_from_slice(self.statements.span_or_empty(found.statements));
            for index in start..sequence.len() {
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
            let scope = MachineScope {
                prior_statements: &sequence,
                ..*machine
            };
            assign_statement_expression_symbols(
                self.symbols,
                &scope,
                parameters,
                state,
                self.expressions,
                self.child_types,
                found.tail,
            );
            self.statements
                .span_mut_or_empty(found.statements)
                .clone_from_slice(&sequence[start..]);
            self.sequence(machine, parameters, state, &sequence, start);
            self.blocks_in(machine, parameters, state, &sequence, found.tail);
        }
    }
}
mod routing {
    use crate::source_symbols::SymbolTable;
    use arena::Arena;
    use symbols::{SymbolHandle, SymbolKind};

    use crate::resolution::names::expressions::{
        assign_expression_span_symbols, assign_statement_expression_symbols,
    };
    use crate::resolution::names::paths::resolve_lexical_members;
    use crate::resolution::names::scopes::MachineScope;
    use crate::resolution::names::targets::{
        assign_call_machine_argument_symbols, assign_transition_target_symbols,
        resolve_call_target_symbol,
    };
    use crate::resolution::names::type_references::assign_type_reference_symbol_with_locals_and_constraints;
    use crate::source_resolved::statement::TransitionGuard;

    pub(super) fn assign_local_type_symbols(
        machine: &MachineScope<'_>,
        parameters: &[crate::source_resolved::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut crate::source_resolved::expression::ExpressionTable,
        child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
        type_constraints: &Arena<crate::source_resolved::types::TypeConstraint>,
        reference: &mut crate::source_resolved::types::TypeReference,
        symbols: &SymbolTable,
    ) {
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            machine.type_parameters,
            reference,
        );
        crate::resolution::names::type_references::assign_type_value_expression_symbols(
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
        statements: &mut [crate::source_resolved::statement::Statement],
        parameters: &[crate::source_resolved::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut crate::source_resolved::expression::ExpressionTable,
        child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
        type_constraints: &arena::Arena<crate::source_resolved::types::TypeConstraint>,
        statement_path_members: &mut Arena<crate::source_resolved::name::DiagnosticName>,
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
        parameters: &[crate::source_resolved::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut crate::source_resolved::expression::ExpressionTable,
        child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
        type_constraints: &arena::Arena<crate::source_resolved::types::TypeConstraint>,
        statement_path_members: &mut Arena<crate::source_resolved::name::DiagnosticName>,
        statement: &mut crate::source_resolved::statement::Statement,
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
            crate::source_resolved::statement::Statement::Assembly(_) => {}
            crate::source_resolved::statement::Statement::RootBinding(binding) => {
                assign_expr!(binding.receiver);
                if binding.implementation_operand.is_valid() {
                    assign_expr!(binding.implementation_operand);
                    let described = match expression_table
                        .expression(binding.implementation_operand)
                    {
                        crate::source_resolved::expression::ExpressionNode::Name(path) => matches!(
                            symbols.get(path.symbol).kind,
                            SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                        ),
                        _ => true,
                    };
                    if !described {
                        binding.implementation_operand =
                            crate::source_resolved::expression::ExpressionHandle::invalid();
                    }
                }
            }
            crate::source_resolved::statement::Statement::AssemblyFact(fact) => {
                assign_expr!(fact.expression);
            }
            crate::source_resolved::statement::Statement::Assignment(assignment) => {
                assign_expr!(assignment.target);
                assign_expr!(assignment.value);
            }
            crate::source_resolved::statement::Statement::Call(call) => {
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
                    let (root, symbol) =
                        resolve_lexical_members(symbols, machine, parameters, members);
                    call.receiver_root_symbol = root;
                    call.receiver_symbol = symbol;
                }

                let value_callee =
                    crate::source_resolved::callable_values::callee_uses_value_binding(
                        symbols,
                        expression_table,
                        call.callee,
                    );
                call.target_symbol = if value_callee {
                    SymbolHandle::invalid()
                } else {
                    crate::resolution::names::targets::normalize_statement_module_call(
                        symbols,
                        statement_path_members,
                        call,
                    )
                    .unwrap_or_else(|| {
                        resolve_call_target_symbol(
                            machine,
                            parameters,
                            !call.receiver.is_empty(),
                            call.receiver_symbol,
                            &call.target,
                            child_type_references,
                            symbols,
                        )
                    })
                };
                call.unresolved_name = if value_callee || call.target_symbol.is_valid() {
                    Default::default()
                } else {
                    crate::resolution::names::expressions::unresolved_call_name(
                        symbols,
                        machine,
                        parameters,
                        statement_path_members.span_or_empty(call.receiver),
                        &call.target,
                    )
                };
                crate::resolution::names::expressions::assign_static_arguments_type_symbols(
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
            crate::source_resolved::statement::Statement::ProofOutputBindingStatement(binding) => {
                binding.machine_symbol = machine.symbol;
                binding.state_symbol = state_symbol;
                assign_expr!(binding.call);
            }
            crate::source_resolved::statement::Statement::Expression(expression)
            | crate::source_resolved::statement::Statement::Evaluate { expression, .. } => {
                assign_expr!(*expression);
            }
            crate::source_resolved::statement::Statement::LocalData(local_data) => {
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
            crate::source_resolved::statement::Statement::Transition(transition) => {
                use crate::source_resolved::expression::{
                    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
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
                for retained in transition.patterns.iter_mut() {
                    if retained.pattern.application_type.is_valid() {
                        crate::resolution::names::type_references::reassign_type_reference_symbols_and_values(
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
                    crate::resolution::names::expressions::assign_pattern_head_symbols(
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
                        let symbol =
                            crate::resolution::names::expression_paths::pattern_field_symbol(
                                symbols,
                                selected,
                                field.member.as_str(),
                            );
                        expression_table.match_field_mut(handle).field_symbol = symbol;
                    }
                    assign_expr!(retained.pattern.condition);
                    if retained.pattern.subject_binding.is_valid()
                        && !retained.pattern.guard.is_valid()
                    {
                        continue;
                    }
                    let mut scalar_value = false;
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
                                domain_arguments: arena::HandleSpan::empty(),
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
                        scalar_value = true;
                        let test = expression_table.insert(ExpressionNode::Binary(
                            TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                negation: false,
                                left: retained.subject,
                                operator: BinaryOperator::Equal,
                                right: retained.pattern.head,
                            },
                        ));
                        // A scalar pattern selects ordinary equality at its
                        // head. Retain that source question so policy and
                        // user declarations use the same selection owner.
                        expression_table.set_source_span(
                            test,
                            expression_table.source_span(retained.pattern.head),
                        );
                        if let Some(exposure) =
                            expression_table.authored_expression_exposure(retained.pattern.head)
                        {
                            expression_table.set_authored_expression_exposure(test, exposure);
                        }
                        if let Some(partition) =
                            expression_table.compiler_selection_partition(retained.pattern.head)
                        {
                            expression_table.set_compiler_selection_partition(test, partition);
                        }
                        Some(test)
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
                                negation: false,
                                left: test,
                                operator: BinaryOperator::And,
                                right: retained.pattern.condition,
                            }))
                        }
                    } else {
                        retained.pattern.condition
                    };
                    if scalar_value {
                        // The comparison now owns the value expression and its
                        // source choice. Later readers consume that condition;
                        // only case/domain patterns retain a nominal head.
                        retained.pattern.head = ExpressionHandle::invalid();
                        retained.pattern.condition = axis;
                    }
                    selected_guard = Some(if let Some(prior) = selected_guard {
                        expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            negation: false,
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
                                negation: false,
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

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_symbols::SymbolTable;

use super::expressions::{assign_expression_span_symbols, assign_statement_expression_symbols};
use super::scopes::MachineScope;

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

fn assign_machine_references(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    selected_bodies: Option<&[symbols::SymbolHandle]>,
    lexical_inputs: &mut [crate::resolution::lowering::state::StateLexicalInputs],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let attached_machines = super::scopes::attached_machines(program);
    let binder_roles = super::scopes::BinderRoles::of(program, symbols);
    let trait_scopes = super::scopes::TraitLexicalScopes::of(program);
    let conformance_scopes = super::scopes::DeclarationTypeScopes::of(program)?;
    let SymbolResolvedTrees {
        roots:
            crate::source_resolved::SymbolResolvedRoots {
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
    let ranking_views = &tables.declarations.ranking_views;
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
    // A Product selects every body, so test membership by binary search
    // rather than scanning the selection once per machine.
    let symbol_key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
    let selected_keys = selected_bodies.map(|selected| {
        let mut keys = selected.iter().copied().map(symbol_key).collect::<Vec<_>>();
        keys.sort_unstable();
        keys
    });
    machines.for_each_mut(|machine| {
        if selected_keys
            .as_ref()
            .is_some_and(|keys| keys.binary_search(&symbol_key(machine.symbol)).is_err())
        {
            return;
        }
        let machine_symbol = machine.symbol;
        let machine_type_parameters = machine.type_parameters;
        let data_definition = data_definitions
            .iter()
            .find(|data_definition| data_definition.symbol == machine.attached_data_symbol);
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let invocation_scope = super::scopes::declaration_type_scope(
            data_type_parameters.span_or_empty(machine_type_parameters),
            conformance_scopes.parameters(machine_symbol),
        );
        let lexical_type_parameters = super::scopes::declaration_type_scope(
            &invocation_scope,
            data_definition.map_or(&[], |owner| {
                data_type_parameters.span_or_empty(owner.type_parameters)
            }),
        );
        let crate::source_resolved::machine::MachineStorage {
            owned_data,
            satisfies: _,
            ranking_subjects,
            ranking_view_arguments,
            ranking_range,
            ranking_view,
            selected_ranking_view,
            contracts: _,
            states,
            ..
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            attached_machines: &attached_machines,
            binder_roles: &binder_roles,
            resolution_indexes: &program.resolution_indexes,
            symbol: machine_symbol,
            lexical_requirements: trait_scopes.machine_requirements(machine.symbol),
            type_parameters: &lexical_type_parameters,
            attached_data_symbol: machine.attached_data_symbol,
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
            prior_statements: &[],
            contract_result_data: symbols::SymbolHandle::invalid(),
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
        *selected_ranking_view = resolve_ranking_view(
            symbols,
            machine_symbol,
            ranking_views.span_or_empty(*ranking_view),
        );
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
            let statements =
                state_statements.span_mut_or_empty(state.construction_statement_span());
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
            .assign(
                &machine_scope,
                parameters,
                state_symbol,
                state.construction_statement_span(),
            );
            blocks::BlockScopeRouting {
                statements: state_statements,
                expressions: expression_table,
                child_types: child_type_references,
                type_constraints,
                path_members: statement_path_members,
                symbols,
            }
            .assign(
                &machine_scope,
                parameters,
                state_symbol,
                state.construction_statement_span(),
            );
        }
    });
    Ok(())
}

/// Ranking names use ordinary declaration precedence. The closed slice-length
/// view is a builtin only when no authored declaration shadows that name.
fn resolve_ranking_view(
    symbols: &SymbolTable,
    machine: symbols::SymbolHandle,
    members: &[crate::source_resolved::name::DiagnosticName],
) -> source_declarations::RankingViewSelection {
    use crate::source_symbols::SymbolLookup;
    use semantic_vocabulary::RankingViewId;
    use source_declarations::RankingViewSelection;
    use symbols::SymbolKind;
    if members.is_empty() {
        return RankingViewSelection::Unresolved;
    }
    let path = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let kinds = &[SymbolKind::Machine, SymbolKind::Measure];
    let lexical = super::lookup::lexical_declaration_lookup(symbols, machine, &path, kinds);
    let selected = match lexical {
        SymbolLookup::NotFound => symbols.lookup_signature_free_top_level_from_source_matching(
            &path,
            kinds,
            super::lookup::diagnostic_path_source_span(members),
            |_| true,
        ),
        found => found,
    };
    match selected {
        SymbolLookup::Unique(declaration) => RankingViewSelection::Declaration(declaration),
        SymbolLookup::NotFound if path == "Slice::Length" => {
            RankingViewSelection::Builtin(RankingViewId::SLICE_LENGTH)
        }
        SymbolLookup::NotFound | SymbolLookup::Ambiguous { .. } => RankingViewSelection::Unresolved,
    }
}

#[cfg(test)]
mod scalar_pattern_selection_tests {
    use crate::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Late,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use crate::source_resolved::expression::{BinaryOperator, ExpressionNode};

    #[test]
    fn scalar_transition_comparisons_retain_the_pattern_source_question() {
        let text = "machine choose(value: u64 in Trapping) -> u8 { transition value { 0 -> (7), _ -> (0) } }";
        let mut sources = crate::source_inputs::SourceMap::default();
        let source = sources
            .add("patterns/scalar.omg".into(), text.into())
            .source_id;
        let tokens = crate::resolution::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
                .unwrap();
        let program = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let expressions = &program.tables.bodies.expressions;
        let comparisons = program
            .machines
            .iter()
            .flat_map(|machine| program.machine_state_handles(machine.states))
            .flat_map(|state| {
                program.construction_statements(
                    program.machine_state(*state).construction_statement_span(),
                )
            })
            .filter_map(|statement| {
                use crate::source_resolved::statement::{Statement, TransitionGuard};
                let Statement::Transition(transition) = statement else {
                    return None;
                };
                let TransitionGuard::When(handle) = transition.guard else {
                    return None;
                };
                let ExpressionNode::Binary(binary) = expressions.expression(handle) else {
                    return None;
                };
                (binary.operator == BinaryOperator::Equal).then_some((handle, binary))
            })
            .collect::<Vec<_>>();
        assert!(!comparisons.is_empty(), "the scalar arm compares its value");
        for (handle, binary) in comparisons {
            let span = expressions.source_span(handle);
            assert_eq!(span, expressions.source_span(binary.right));
            assert_eq!(&text[span.span.start..span.span.end], "0");
            assert!(expressions.authored_expression_exposure(handle).is_some());
            let occurrences = expressions
                .authored_selection_occurrences(handle)
                .collect::<Vec<_>>();
            let [occurrence] = occurrences.as_slice() else {
                panic!("each scalar comparison has one ordinary operator question: {occurrences:?}")
            };
            let selected = program
                .authored_declaration_selections()
                .get(*occurrence)
                .unwrap();
            assert_eq!(selected.kind(), Kind::Operator);
            assert_eq!(selected.source_span(), span);
            assert_eq!(selected.target(), Target::LateBound(Late::CheckedOperator));
        }
    }
}
