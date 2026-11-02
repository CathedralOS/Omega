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

/// Assigns symbols across a statement slice so each statement sees only its
/// stamped predecessors: receivers and locals may name earlier statements,
/// never their own binding or a later one.
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
        // The enclosing state routes graph instructions and contracts under
        // their exact outer prefix in `assign_assembly_scope_symbols`.
        symbol_resolved_trees::statement::Statement::Assembly(_) => {}
        symbol_resolved_trees::statement::Statement::RootBinding(binding) => {
            assign_expr!(binding.receiver);
            if binding.implementation_operand.is_valid() {
                assign_expr!(binding.implementation_operand);
                // The operand stays delegated only when its bare name resolves
                // to a value place -- the described `ProductEntryRef` handoff.
                // Any declaration resolution (machine, module, data, const)
                // falls back to the lexical product path in `implementation`.
                let described = match expression_table.expression(binding.implementation_operand) {
                    symbol_resolved_trees::expression::ExpressionNode::Name(path) => matches!(
                        symbols.get(path.symbol).kind,
                        SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                    ),
                    // Computed operands resolve in the ordinary build context;
                    // only a name can denote the static product-path channel.
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
                // A projected member may await type-aware resolution. Its
                // lexical root is already exact and must survive independently.
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
            // The statement `Call` wrapper keeps `target` beside its storage row;
            // borrow the two apart so the router can read one and mutate the other.
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
                BinaryOperator, ExpressionNode, TableBinaryExpression, TableMembershipExpression,
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
                    let field =
                        &expression_table.match_fields(retained.pattern.fields)[position as usize];
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
                    Some(
                        expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                            left: retained.subject,
                            operator: BinaryOperator::Equal,
                            right: retained.pattern.head,
                        })),
                    )
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
