//! Named boundary transitions are terminal value calls, not machine-state
//! jumps. Normalize their complete declaration path into the ordinary call
//! representation before result-overload selection. Operand checks, effects,
//! requires contracts, result compatibility and provider selection then share
//! the expression path; a qualified name alone never admits execution.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};
use typed_trees::statement::TransitionTargetNode;

pub(super) fn normalize_boundary_transition_calls(program: &mut TypedTrees) {
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
                    // An already resolved machine/state identity stays a jump.
                    if path.symbol.is_valid()
                        && program.operator_definition_by_symbol(path.symbol).is_none()
                    {
                        continue;
                    }
                    let members = program.statement_table.name_path_members(path.members);
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
                        )
                    {
                        continue;
                    }
                    if program.operators().iter().any(|operator| {
                        operator.is_boundary
                            && operator.spelling.is_none()
                            && program.operator_path_members(operator.name) == members
                    }) && !targets.contains(&target)
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
                    head_symbol: path.head_symbol,
                    symbol: if namespace.count() == 1 {
                        path.head_symbol
                    } else {
                        SymbolHandle::invalid()
                    },
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
