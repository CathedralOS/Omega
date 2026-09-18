use super::super::SymbolHandle;
use super::{
    CheckedBooleanExpression, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedScalarExpressionRole, ExpressionNode, StatementNode,
};
use crate::values::build_checked_scalar_computation_plans;
use crate::values::scalar::computations::tests::checked_source;

#[test]
fn scalar_computations_keep_assignment_rhs_and_prior_storage_namespace() {
    let checked = checked_source(
        r#"
        machine identity(input: bool) -> bool { input }
        machine value(flag: bool) -> bool {
            let mut current: bool = flag;
            let saved: bool = current;
            current = current && identity(saved);
            let after: bool = saved;
            current = !current;
            after || current
        }
        "#,
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(current) = &statements[0] else {
        panic!("storage declaration");
    };
    let StatementNode::Assignment(assignment) = &statements[2] else {
        panic!("authored assignment");
    };
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    let root = roots[0];
    assert_eq!(root.state, state.symbol);
    assert_eq!(root.statement_ordinal, 2);
    assert_eq!(root.role, CheckedScalarExpressionRole::AssignmentValue);
    assert_eq!(plans.nodes.get(root.root).authored_root, assignment.value);
    let CheckedScalarComputationKind::Select {
        condition,
        when_true,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        panic!("selective assignment RHS");
    };
    assert_eq!(
        plans.nodes.get(condition).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StorageRead {
                symbol: current.symbol
            },
        )))
    );
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(when_true).kind
    else {
        panic!("selected RHS call");
    };
    let argument = plans.operands.span_or_empty(arguments)[0];
    assert_eq!(
        plans.nodes.get(argument).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Local { position: 1 },
        )))
    );
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .unwrap();
    assert_eq!(
        graph.states[0].bindings[2].destination,
        checked_trees::CheckedScalarBindingDestination::StorageAssign {
            symbol: current.symbol
        }
    );
    assert_eq!(
        graph.states[0].bindings[2].value,
        checked_trees::CheckedScalarBindingValue::Computation
    );
    assert_eq!(
        graph.states[0].bindings[4].value,
        checked_trees::CheckedScalarBindingValue::Expression
    );
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            state.symbol,
            3,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        ),
        Some(&CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Local { position: 1 },
        )))
    );
}

#[test]
fn scalar_computations_retain_nested_assignment_calls() {
    let checked = checked_source(
        "machine identity(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             let mut current: bool = flag;
             current = identity(identity(current));
             current
         }",
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].role, CheckedScalarExpressionRole::AssignmentValue);
    let CheckedScalarComputationKind::Call {
        arguments,
        call_ordinal: outer,
        ..
    } = plans.nodes.get(roots[0].root).kind
    else {
        panic!("outer assignment call");
    };
    let CheckedScalarComputationKind::Call {
        call_ordinal: inner,
        ..
    } = plans
        .nodes
        .get(plans.operands.span_or_empty(arguments)[0])
        .kind
    else {
        panic!("inner assignment call");
    };
    assert!(outer < inner, "authored preorder identity is retained");
}

#[test]
fn scalar_computations_do_not_duplicate_pure_assignment_roots() {
    let checked = checked_source(
        "machine value(flag: bool) -> bool {
             let mut current: bool = flag;
             current = !current;
             current
         }",
        false,
    );
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .next()
            .is_none()
    );
    assert_eq!(
        checked.facts.flow.terminal_scalar_graphs.machines[0].states[0].bindings[1].value,
        checked_trees::CheckedScalarBindingValue::Expression
    );
}

#[test]
fn scalar_computations_refuse_nonlocal_or_unestablished_assignment_destinations() {
    let checked = checked_source(
        "machine identity(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             let saved: bool = flag;
             let mut current: bool = flag;
             current = identity(current);
             let mut later: bool = flag;
             current || later
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(saved) = &statements[0] else {
        panic!("immutable local");
    };
    let StatementNode::LocalData(later) = &statements[3] else {
        panic!("future local");
    };
    let StatementNode::Assignment(assignment) = &statements[2] else {
        panic!("assignment");
    };
    let parameter = checked.state_parameters(state)[0].symbol;
    for destination in [
        parameter,
        saved.symbol,
        later.symbol,
        SymbolHandle::invalid(),
    ] {
        let mut changed = checked.typed.clone();
        let ExpressionNode::Name(name) = changed.expression_table.expression_mut(assignment.target)
        else {
            panic!("bare destination name");
        };
        name.symbol = destination;
        let plans = build_checked_scalar_computation_plans(
            &changed,
            &checked.facts.operators,
            &checked.facts.flow,
            &checked.facts.borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            plans
                .root_at(
                    state.symbol,
                    2,
                    CheckedScalarExpressionRole::AssignmentValue
                )
                .is_none(),
            "destination={destination:?}"
        );
    }
    let mut changed = checked.typed.clone();
    let StatementNode::Assignment(target) = &mut changed
        .statement_table
        .statements_mut(state.statement_nodes)[2]
    else {
        panic!("assignment");
    };
    target.target = assignment.value;
    let plans = build_checked_scalar_computation_plans(
        &changed,
        &checked.facts.operators,
        &checked.facts.flow,
        &checked.facts.borrow,
        &checked.facts.proof,
        &checked.facts.values.scalar_expressions,
        &[],
    );
    assert!(
        plans
            .root_at(
                state.symbol,
                2,
                CheckedScalarExpressionRole::AssignmentValue
            )
            .is_none()
    );
}

#[test]
fn scalar_computations_keep_indexed_store_call_value() {
    let checked = checked_source(
        r#"
        machine take() -> u8 { 7 }
        data Main { cells: [u8; 8]; }
        machine Main::main(&mut self) {
            let i: u64 = 2;
            self.cells[i] = take();
        }
        "#,
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::Assignment(assignment) = &statements[1] else {
        panic!("indexed assignment");
    };
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| root.state == state.symbol)
        .collect::<Vec<_>>();
    // The call-valued RHS keeps the same `AssignmentValue` coordinate a
    // `Name`/`Member` store uses; the local-read index stays in the pure plan.
    let [value_root] = roots.as_slice() else {
        panic!("only the call value retains a computation root")
    };
    assert_eq!(value_root.statement_ordinal, 1);
    assert_eq!(
        value_root.role,
        CheckedScalarExpressionRole::AssignmentValue
    );
    let value_node = plans.nodes.get(value_root.root);
    assert_eq!(value_node.authored_root, assignment.value);
    assert!(matches!(
        value_node.kind,
        CheckedScalarComputationKind::Call { .. }
    ));
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                1,
                CheckedScalarExpressionRole::AssignmentIndex,
            )
            .is_some()
    );
}

#[test]
fn scalar_computations_do_not_duplicate_pure_indexed_store_operands() {
    let checked = checked_source(
        r#"
        data Main { cells: [u8; 8]; }
        machine Main::main(&mut self) {
            let i: u64 = 2;
            self.cells[i] = 7;
        }
        "#,
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    // Both operands are already pure rows at the same coordinates; no
    // computation duplicates them.
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.state == state.symbol)
            .is_none()
    );
}
