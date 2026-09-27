use super::{BOOLEAN_HELPERS, artifact, source};
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn returned_boundary_computations_reject_outer_and_nested_source_custody_drift() {
    let checked = crate::front_end::checked_program(&source(
        "bool",
        "identity(false), Scalar::identity(identity(true))",
        BOOLEAN_HELPERS,
        false,
    ));
    artifact(&checked);
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &checked.typed.machine_states(root)[0];
    let StatementNode::LocalData(local) = &checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        unreachable!();
    };
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(local.initial_value)
    else {
        unreachable!();
    };
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    let receipt = &checked.typed.state_parameters(state)[0];
    let (outer_flow, _) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, flow)| flow.authored_expression == local.initial_value && flow.call_ordinal == 0)
        .unwrap();
    for mutation in 0..11 {
        let mut changed = checked.clone();
        match mutation {
            0 => {
                let ExpressionNode::Call(call) = changed
                    .typed
                    .expression_table
                    .expression_mut(local.initial_value)
                else {
                    unreachable!();
                };
                call.target_symbol = symbols::SymbolHandle::invalid();
            }
            1 => changed
                .typed
                .expression_table
                .set_expression_handle_at_offset(call.arguments, 0, arguments[1]),
            2 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } =
                    &mut plan.boundary_call
                else {
                    unreachable!();
                };
                coordinate.call_ordinal += 1;
            }
            3 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .return_statement_ordinal += 1
            }
            4 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &mut plan.boundary_call
                else {
                    unreachable!();
                };
                scalar_arguments.swap(0, 1);
            }
            5 | 10 => {
                let StatementNode::LocalData(changed_local) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[0]
                else {
                    unreachable!();
                };
                if mutation == 5 {
                    changed_local.type_reference = receipt.type_reference;
                } else {
                    changed_local.symbol = symbols::SymbolHandle::invalid();
                }
            }
            6 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .result_type = typed_trees::types::PrimitiveType::U16;
            }
            7 => {
                let StatementNode::Expression(returned) = checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)[1]
                else {
                    unreachable!();
                };
                let ExpressionNode::Name(path) =
                    changed.typed.expression_table.expression_mut(returned)
                else {
                    unreachable!();
                };
                path.symbol = receipt.symbol;
            }
            8 | 9 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(outer_flow)
                    .authored_expression = if mutation == 8 {
                    arena::Handle::invalid()
                } else {
                    arena::Handle::from_parts(
                        local.initial_value.arena_index(),
                        local.initial_value.generation() + 1,
                    )
                };
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "outer returned-boundary mutation={mutation}"
        );
    }
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root_plan)| {
            root_plan.machine == root.symbol
                && matches!(
                    root_plan.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    for (handle, plan) in roots {
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(handle)
            .statement_ordinal += 1;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(plan.root)
            .authored_root = local.initial_value;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
    }
    for (_, node) in computations.nodes.iter() {
        let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
            continue;
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(source_call)
            .authored_expression = arena::Handle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            unreachable!();
        };
        if call.receiver.is_valid() {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(call.receiver)
            else {
                unreachable!();
            };
            path.symbol = symbols::SymbolHandle::invalid();
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Root::enter")
                )
                .is_err()
            );
        }
    }
}
