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

pub(super) fn normalize_terminal_transition_calls(program: &mut TypedTrees) {
    let mut targets = Vec::new();
    for machine in program.machines() {
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
                    // Attached machines require an explicit value-call spelling
                    // for a bare foreign-machine name. Keep that independent
                    // source-admission refusal visible to its validator.
                    if callable
                        && machine.attached_data.is_some()
                        && members.len() == 1
                        && (program.machine_by_symbol(path.symbol).is_some()
                            || program.machine_holding_state(path.symbol).is_some())
                    {
                        continue;
                    }
                    if path.symbol.is_valid()
                        && !callable
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
        // An owned implementation's machine is allocated at the root. Its
        // semantic namespace is the owning conformance, not that allocation
        // parent. Rejoin only the retained owned-realization relationship;
        // a referenced row or a conflicting lexical head lends no namespace.
        let owned_namespace = program
            .machine_by_symbol(path.symbol)
            .or_else(|| program.machine_holding_state(path.symbol))
            .map(|machine| program.declaration_scope_parent(machine.symbol))
            .filter(|owner| program.conformance_by_symbol(*owner).is_some());
        let namespace_head = path.head_symbol;
        let namespace_endpoint = if members.len() == 1 {
            namespace_head
        } else {
            owned_namespace.unwrap_or_default()
        };
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
