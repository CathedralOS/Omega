//! A boundary application nested under another operand (`a * b + c`) is its
//! own computation node: operands in authored order, one exact call
//! occurrence, consumed once by the statement's outer result operation.
use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

use crate::tests::flow::terminal_unit::{checked, machine_named};

const SOURCE: &str = "data Root { first: f64; second: f64; }
    pub boundary machine + Float::add(left: f64, right: f64) -> f64;
    pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
    pub data F64 {}
    pub boundary operator F64::from_i32(value: i32) -> f64
    ensures true;
    machine Root::enter(&mut self, a: f64, b: f64, count: i32) {
        let product_sum: f64 = a * b + F64::from_i32(count);
        self.first = product_sum;
        self.second = a * (b + a);
    }";

#[test]
fn boundary_applications_feed_mutable_scalar_storage_in_source_order() {
    let checked = checked(
        "data Root {}
        pub boundary machine + Float::add(left: f64, right: f64) -> f64;
        pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
        machine consume(value: f64) {}
        machine Root::enter(left: f64, right: f64) {
            let mut value: f64 = left * right + left;
            value = value * right;
            transition { _ -> finish(value) }
            state finish(value: f64) { consume(value); }
        }",
    );
    let symbol = machine_named(&checked, "Root::enter");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let computations = &checked.facts.values.scalar_computations;
    for (ordinal, role) in [
        (0, CheckedScalarExpressionRole::StorageInitializer),
        (1, CheckedScalarExpressionRole::AssignmentValue),
    ] {
        let root = computations
            .root_at(state.symbol, ordinal, role)
            .expect("each scalar storage write owns its computation");
        let CheckedScalarComputationKind::BoundaryCall { source_call, .. } =
            computations.nodes.get(root.root).kind
        else {
            panic!("the storage computation evaluates its boundary application");
        };
        assert_eq!(
            checked
                .facts
                .flow
                .control
                .calls
                .get(source_call)
                .statement_index,
            ordinal as usize
        );
    }
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(symbol)
            .is_some(),
        "{:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(symbol)
    );
    let initializer = computations
        .root_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::StorageInitializer,
        )
        .unwrap()
        .root;
    let assignment = computations
        .root_at(
            state.symbol,
            1,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .unwrap()
        .root;
    let CheckedScalarComputationKind::BoundaryCall {
        arguments,
        source_call,
        ..
    } = computations.nodes.get(initializer).kind
    else {
        panic!("initializer call");
    };
    let product = computations.operands.span(arguments).unwrap()[0];
    assert!(matches!(
        computations.nodes.get(product).kind,
        CheckedScalarComputationKind::BoundaryCall {
            call_ordinal: 1,
            ..
        }
    ));
    for corruption in 0..3 {
        let mut corrupted = checked.clone();
        let node = corrupted
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(assignment);
        match corruption {
            0 => {
                let CheckedScalarComputationKind::BoundaryCall {
                    source_call: retained,
                    ..
                } = &mut node.kind
                else {
                    panic!("assignment call");
                };
                *retained = source_call;
            }
            1 => node.authored_root = arena::Handle::invalid(),
            _ => node.primitive_type = typed_trees::types::PrimitiveType::F32,
        }
        let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &corrupted.typed,
            &corrupted.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &corrupted.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &corrupted.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            plans.composed_for_machine(symbol).is_none(),
            "substituted occurrence, expression, or carrier must reject: {corruption}"
        );
    }
}

fn argument_root(
    checked: &checked_trees::CheckedTrees,
    statement: u32,
    argument_ordinal: u32,
) -> checked_trees::CheckedScalarComputationHandle {
    checked
        .facts
        .values
        .scalar_computations
        .root_at(
            checked.machine_states(
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "Root::enter")
                    .unwrap(),
            )[0]
            .symbol,
            statement,
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: 0,
                argument_ordinal,
            },
        )
        .unwrap_or_else(|| panic!("statement {statement} argument {argument_ordinal} root"))
        .root
}

/// The authored argument expressions of the call a node applies.
fn authored_arguments(
    checked: &checked_trees::CheckedTrees,
    node: checked_trees::CheckedScalarComputationHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let plans = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { source_call, .. } = plans.nodes.get(node).kind
    else {
        panic!("boundary application node");
    };
    let authored = checked
        .facts
        .flow
        .control
        .calls
        .get(source_call)
        .authored_expression;
    let ExpressionNode::Call(call) = checked.expression_table.expression(authored) else {
        panic!("authored call");
    };
    checked
        .expression_table
        .expression_handles(call.arguments)
        .to_vec()
}

fn operand_sources(
    checked: &checked_trees::CheckedTrees,
    node: checked_trees::CheckedScalarComputationHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let plans = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { arguments, .. } = plans.nodes.get(node).kind
    else {
        panic!("boundary application node");
    };
    plans
        .operands
        .span(arguments)
        .unwrap()
        .iter()
        .map(|operand| plans.nodes.get(*operand).authored_root)
        .collect()
}

fn replanned_machine(checked: &checked_trees::CheckedTrees) -> bool {
    crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    )
    .for_machine(machine_named(checked, "Root::enter"))
    .is_some()
}

#[test]
fn nested_applications_are_ordered_single_occurrence_operands() {
    let checked = checked(SOURCE);
    let symbol = machine_named(&checked, "Root::enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .is_some(),
        "{:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(symbol)
    );
    let plans = &checked.facts.values.scalar_computations;
    // `a * b + F64::from_i32(count)`: both operands of the outer sum are
    // nested applications, a machine declaration and a named operator, at
    // their own preorder occurrences.
    for (argument, ordinal) in [(0, 1), (1, 2)] {
        let node = argument_root(&checked, 0, argument);
        assert!(
            matches!(
                plans.nodes.get(node).kind,
                CheckedScalarComputationKind::BoundaryCall { call_ordinal, .. }
                    if call_ordinal == ordinal
            ),
            "argument {argument} is occurrence {ordinal}"
        );
        assert_eq!(
            operand_sources(&checked, node),
            authored_arguments(&checked, node),
            "operands keep authored order"
        );
    }
    // `self.second = a * (b + a)`: the field store's grouped sum.
    let grouped = argument_root(&checked, 2, 1);
    assert_eq!(
        operand_sources(&checked, grouped),
        authored_arguments(&checked, grouped)
    );
    // The statements' own outer calls stay result operations, not roots.
    let state = checked.machine_states(
        checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
            .unwrap(),
    )[0]
    .clone();
    let statements = checked.statement_table.statements(state.statement_nodes);
    assert!(matches!(statements[0], StatementNode::LocalData(_)));
    for (statement, role) in [
        (
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        ),
        (2, CheckedScalarExpressionRole::AssignmentValue),
    ] {
        assert!(
            plans.root_at(state.symbol, statement, role).is_none(),
            "statement {statement} keeps its outer application as an operation"
        );
    }
}

#[test]
fn reordered_or_duplicated_application_occurrences_reject() {
    let checked = checked(SOURCE);
    assert!(replanned_machine(&checked), "unchanged facts replan");
    let product = argument_root(&checked, 0, 0);
    let grouped = argument_root(&checked, 2, 1);

    // Duplicated: the field store's sum claims the first product's call.
    let mut duplicated = checked.clone();
    let plans = &mut duplicated.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { source_call, .. } =
        plans.nodes.get(product).kind
    else {
        panic!("product application");
    };
    let CheckedScalarComputationKind::BoundaryCall {
        source_call: claimed,
        ..
    } = &mut plans.nodes.get_mut(grouped).kind
    else {
        panic!("grouped application");
    };
    *claimed = source_call;
    assert!(
        !replanned_machine(&duplicated),
        "a duplicated occurrence rejects"
    );

    // Reordered: the product claims the conversion's later occurrence slot.
    let mut reordered = checked.clone();
    let CheckedScalarComputationKind::BoundaryCall { call_ordinal, .. } = &mut reordered
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(product)
        .kind
    else {
        panic!("product application");
    };
    *call_ordinal = 2;
    assert!(
        !replanned_machine(&reordered),
        "a reordered occurrence rejects"
    );
}

/// A literal operand of a legacy named operator lands at the operator's
/// formal, so `F64::minimum(a * b, 2.5)` carries a pure literal beside its
/// nested application instead of losing the whole statement plan.
#[test]
fn named_operator_literal_operands_land_beside_nested_applications() {
    let checked = checked(
        "data Root { first: f64; }
        pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
        pub data F64 {}
        pub boundary operator F64::minimum(left: f64, right: f64) -> f64
        ensures true;
        machine Root::enter(&mut self, a: f64, b: f64) {
            self.first = F64::minimum(a * b, 2.5);
        }",
    );
    let symbol = machine_named(&checked, "Root::enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .is_some(),
        "{:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(symbol)
    );
    let state = checked.machine_states(
        checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
            .unwrap(),
    )[0]
    .symbol;
    let role = |argument_ordinal| CheckedScalarExpressionRole::BoundaryCallArgument {
        call_ordinal: 0,
        argument_ordinal,
    };
    assert!(matches!(
        checked
            .facts
            .values
            .scalar_computations
            .root_at(state, 0, role(0))
            .map(|root| &checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root)
                .kind),
        Some(CheckedScalarComputationKind::BoundaryCall {
            call_ordinal: 1,
            ..
        })
    ));
    assert!(matches!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(state, 0, role(1)),
        Some(checked_trees::CheckedScalarExpression::IeeeFloatLiteral { .. })
    ));
}
