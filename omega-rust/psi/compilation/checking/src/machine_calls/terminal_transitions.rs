//! Named terminal invocations use ordinary call results, while local states remain
//! control edges. Normalize the complete declaration path into the ordinary call
//! representation before result-overload selection. Operand checks, effects,
//! requires contracts, result compatibility and provider selection then share
//! the expression path; a qualified name alone never admits execution.

use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};
use typed_trees::statement::TransitionTargetNode;

pub fn normalize_terminal_transition_calls(
    program: &mut TypedTrees,
    first_machine: usize,
    completed_machines: &[symbols::SymbolHandle],
) {
    let mut targets = Vec::new();
    for (index, machine) in program.machines().iter().enumerate() {
        if index < first_machine && !completed_machines.contains(&machine.symbol) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let TransitionTargetNode::Named { path, .. } =
                        program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    let members = program.statement_table.name_path_members(path.members);
                    if members.is_empty() {
                        continue;
                    }
                    let callable =
                        crate::terminal_callable_target(program, machine, path.symbol).is_some();
                    // A named implementation is code, not a state container.
                    // Its exact scoped head permits ordinary member binding,
                    // even before a selected member receives its final symbol.
                    let implementation_namespace = !path.symbol.is_valid()
                        && path.receiver_symbol.is_valid()
                        && matches!(
                            program.symbols.get(path.receiver_symbol).kind,
                            SymbolKind::Conformance | SymbolKind::ConformanceParameter
                        );
                    if path.symbol.is_valid()
                        && !callable
                        && !implementation_namespace
                        && program.operator_definition_by_symbol(path.symbol).is_none()
                    {
                        continue;
                    }
                    // A resolved value head is not a declaration namespace, even
                    // if its source spelling matches a boundary family.
                    if members.len() > 1
                        && path.head_symbol.is_valid()
                        && !matches!(
                            program.symbols.get(path.head_symbol).kind,
                            SymbolKind::Module
                                | SymbolKind::BuiltinType
                                | SymbolKind::Data
                                | SymbolKind::Domain
                                | SymbolKind::Machine
                                | SymbolKind::Trait
                                | SymbolKind::Conformance
                                | SymbolKind::ConformanceParameter
                        )
                    {
                        continue;
                    }
                    if (callable
                        || implementation_namespace
                        || program.operators().iter().any(|operator| {
                            operator.is_boundary
                                && operator.spelling.is_none()
                                && program.operator_path_members(operator.name) == members
                        }))
                        && !targets.contains(&target)
                    {
                        targets.push(target);
                    }
                }
            }
        }
    }
    for target in targets {
        let TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
            source_span,
            authored_call_selection,
            static_machine_parameter,
            operational_acknowledgement,
        } = program.statement_table.transition_target(target).clone()
        else {
            unreachable!("collected boundary transition changed shape");
        };
        let mut members = program
            .statement_table
            .name_path_members(path.members)
            .to_vec();
        let leaf = members.pop().expect("boundary declaration has a path");
        // Both ends come from source resolution. Neither package aliases nor
        // root-allocated implementation machines recreate a missing qualifier.
        let namespace_head = path.head_symbol;
        let namespace_endpoint = path.receiver_symbol;
        let receiver = if members.is_empty() {
            ExpressionHandle::invalid()
        } else {
            let mut namespace = arena::HandleSpan::empty();
            for member in members {
                program
                    .expression_table
                    .push_name_path_member(&mut namespace, member);
            }
            let receiver = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: namespace,
                    member_symbols: arena::HandleSpan::empty(),
                    head_symbol: namespace_head,
                    symbol: namespace_endpoint,
                }));
            program
                .expression_table
                .set_source_span(receiver, source_span);
            receiver
        };
        let operands = program
            .statement_table
            .expression_handles(arguments)
            .to_vec();
        let arguments = program.expression_table.insert_expression_handles(operands);
        let call = program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                authored_machine_application: Default::default(),
                requirement_application: Default::default(),
                exact_coercion_target: Default::default(),
                receiver,
                target_symbol: path.symbol,
                static_machine_parameter,
                target: leaf,
                static_requirement_dispatch: None,
                machine_arguments: Default::default(),
                quotient_operation: None,
                private_layout_operation: None,
                layout_query: None,
                native_code_layout: None,
                build_product_request: Default::default(),
                arguments,
                evidence_arguments,
                operational_acknowledgement,
            }));
        program.expression_table.set_source_span(call, source_span);
        if let Some(occurrence) = authored_call_selection {
            program
                .expression_table
                .attach_authored_selection_occurrences(call, [occurrence]);
        }
        *program.statement_table.transition_target_mut(target) = TransitionTargetNode::Value(call);
    }
}
