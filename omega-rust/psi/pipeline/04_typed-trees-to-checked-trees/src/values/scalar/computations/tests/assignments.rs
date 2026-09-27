use super::super::SymbolHandle;
use super::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, ExpressionNode, StatementNode,
};
use crate::values::build_checked_scalar_computation_plans;
use crate::values::scalar::computations::tests::checked_source;

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

/// A builtin selection is an ordinary integer computation. It has to compose as
/// an operand of arithmetic, not only as a whole assigned value: a builtin has
/// no entry state, so the ordinary call route cannot resolve it.
#[test]
fn a_builtin_selection_composes_as_an_arithmetic_operand() {
    let checked = checked_source(
        r#"
        data Main { lo: i32; hi: i32; mid: i32; }
        machine Main::main(&mut self) {
            self.mid = (min(max(self.lo, 0), 31) + min(max(self.hi, 0 - 1), 30)) / 2;
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
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .expect("the clamped midpoint retains a computation root");
    assert!(matches!(
        plans.nodes.get(root.root).kind,
        CheckedScalarComputationKind::Apply { .. }
    ));
}
