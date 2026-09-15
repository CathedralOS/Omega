//! Selected boundary comparisons bind computation nodes that keep the exact
//! operator use; their published/surviving crash routes stay joined through
//! that use rather than through an invented call or a builtin expression.

use super::checked_source;
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedScalarExpressionRole,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::types::PrimitiveType;

fn return_selected_comparison(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> (
    CheckedScalarComputationHandle,
    arena::Handle<checked_trees::CheckedOperatorUseFact>,
) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("{machine_name} machine"));
    let state = &checked.machine_states(machine)[0];
    let root = checked
        .facts
        .values
        .scalar_computations
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .unwrap_or_else(|| panic!("{machine_name} return computation root"));
    let &CheckedScalarComputationKind::SelectedComparison { operator_use, .. } = &checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(root.root)
        .kind
    else {
        panic!("{machine_name} return is a selected comparison");
    };
    (root.root, operator_use)
}

#[test]
fn selected_integer_comparison_binds_exact_use_and_authored_operands() {
    let checked = checked_source(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         machine compare(left: i32, right: i32) -> bool { right == left }
         pub machine wrapper(left: i32, right: i32) -> bool crashes Trap { compare(left, right) }",
        false,
    );
    let operator = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["Comparison", "equal"])
        })
        .expect("Comparison::equal requirement");
    let (root, operator_use) = return_selected_comparison(&checked, "compare");
    let plans = &checked.facts.values.scalar_computations;
    let &CheckedScalarComputationKind::SelectedComparison { left, right, .. } =
        &plans.nodes.get(root).kind
    else {
        unreachable!()
    };
    let selected = checked.facts.operators.uses.get(operator_use);
    assert_eq!(selected.selected_operator_symbol, operator.symbol);
    assert_eq!(
        selected.status,
        checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
    let ExpressionNode::Binary(binary) = checked
        .typed
        .expression_table
        .expression(selected.expression)
    else {
        panic!("selected use is the authored binary expression")
    };
    let operands = selected
        .operands(&checked.typed)
        .expect("two authored operands");
    assert_eq!(operands, vec![binary.left, binary.right]);
    // Authored order is retained: the left operand computation produces the
    // `right` parameter, the right operand computation produces `left`.
    let operand_positions = [left, right].map(|operand| {
        let node = plans.nodes.get(operand);
        assert_eq!(node.primitive_type, PrimitiveType::I32);
        let CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }) = &node.kind
        else {
            panic!("operand computation is a source parameter read")
        };
        assert_eq!(*primitive_type, PrimitiveType::I32);
        *position
    });
    assert_eq!(operand_positions, [1, 0]);
    assert_eq!(
        [left, right].map(|operand| plans.nodes.get(operand).value_source),
        [binary.left, binary.right]
    );
}

#[test]
fn selected_integer_comparison_keeps_its_published_and_surviving_routes() {
    let checked = checked_source(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine safe(value: i32) -> bool requires value >= 0 { 1 == value }
         machine may_crash(left: i32, right: i32) -> bool { left == right }
         pub machine wrapper(left: i32, right: i32) -> bool crashes Trap { may_crash(left, right) }",
        false,
    );
    let mut sites = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators().iter())
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| site.operator_use.arena_index());
    assert_eq!(sites.len(), 2);
    for machine_name in ["safe", "may_crash"] {
        let (_, operator_use) = return_selected_comparison(&checked, machine_name);
        let site = sites
            .iter()
            .find(|site| site.operator_use == operator_use)
            .unwrap_or_else(|| panic!("{machine_name} crash site joins the computation use"));
        assert!(!site.published.is_empty());
        assert_eq!(
            site.surviving.is_empty(),
            machine_name == "safe",
            "{machine_name}: the entry guard discharges the route only under its requires"
        );
    }
}

#[test]
fn unselected_or_mismatched_comparisons_bind_no_selected_computation() {
    for source in [
        // A builtin integer comparison keeps its ordinary pure expression.
        "machine compare(left: i32, right: i32) -> bool { left == right }",
        // A Boolean boundary equality is not an integer comparison.
        "boundary operator == Comparison::equal(left: bool, right: bool) -> bool
         crashes Trap left;
         machine compare(left: bool, right: bool) -> bool { left == right }",
        // A non-boundary selected operator is not a boundary comparison use.
        "operator == Helpers::equal(left: i32, right: i32) -> bool;
         machine compare(left: i32, right: i32) -> bool { left == right }",
        // A non-comparison spelling never becomes a selected comparison even
        // when a crash-qualified boundary requirement supplies it.
        "boundary operator + Comparison::add(left: i32, right: i32) -> i32
         crashes Trap;
         machine compare(left: i32, right: i32) -> bool { (left + right) == 0 }",
    ] {
        let checked = checked_source(source, false);
        assert!(
            checked
                .facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .all(|(_, node)| !matches!(
                    node.kind,
                    CheckedScalarComputationKind::SelectedComparison { .. }
                )),
            "{source}"
        );
    }
}
