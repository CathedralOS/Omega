use super::CheckedScalarExpressionRole;
use crate::values::scalar::computations::tests::checked_source;
use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Row-major authored leaves of an array literal, the order the planner owes.
fn authored_leaves(
    checked: &checked_trees::CheckedTrees,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    match checked.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(elements) => checked
            .expression_table
            .expression_handles(*elements)
            .iter()
            .flat_map(|element| authored_leaves(checked, *element))
            .collect(),
        _ => vec![expression],
    }
}

#[test]
fn supplied_array_members_plan_row_major_leaves_keyed_by_their_array() {
    let checked = checked_source(
        r#"
data Pair { left: [i32; 3]; right: [i32; 2]; }
data Grid { cells: [[i16; 2]; 2]; tag: u8; }
data Framed { pair: Pair; grid: Grid; }
machine build(seed: i32) -> i32 {
    let framed: Framed = Framed {
        pair: Pair { left: [seed, 0 - 2, 7], right: [5, seed] },
        grid: Grid { cells: [[1, 2], [3, 0 - 4]], tag: 9 }
    };
    seed
}
"#,
        false,
    );
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("the supplied-array construction retains its unit plan");
    assert!(
        plan.operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.statement_index == 0
        )),
        "the local establishes one structural value"
    );
    let values = &checked.facts.values.structural_values;
    let computations = &checked.facts.values.scalar_computations;
    let arrays = values
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            CheckedStructuralValueKind::ScalarArray { elements } => {
                Some((node.expression, elements.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        arrays
            .iter()
            .map(|(_, elements)| elements.len())
            .collect::<Vec<_>>(),
        [3, 2, 4],
        "two arrays in one literal and a nested array each keep their own roster"
    );
    for (expression, elements) in &arrays {
        let leaves = authored_leaves(&checked, *expression);
        assert_eq!(leaves.len(), elements.len());
        for (ordinal, (element, leaf)) in elements.iter().zip(&leaves).enumerate() {
            let root = computations
                .root_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::StructuralArrayElement {
                        expression: *expression,
                        element_ordinal: u32::try_from(ordinal).unwrap(),
                    },
                )
                .expect("each leaf has one role keyed by its own array");
            assert_eq!(root.root, *element);
            assert_eq!(computations.nodes.get(*element).authored_root, *leaf);
        }
    }
    // The negative operand keeps its own authored subtraction as the leaf.
    let (left, left_elements) = &arrays[0];
    let negative = authored_leaves(&checked, *left)[1];
    assert!(matches!(
        checked.expression_table.expression(negative),
        ExpressionNode::Binary(_)
    ));
    assert_eq!(
        computations.nodes.get(left_elements[1]).authored_root,
        negative
    );
    // Each array is a structural member of its own record literal.
    let members = values
        .record_fields
        .iter()
        .filter_map(|(_, field)| match field.value {
            CheckedStructuralRecordFieldValue::Structural(value) => {
                Some(values.nodes.get(value).expression)
            }
            CheckedStructuralRecordFieldValue::Scalar(_) => None,
        })
        .collect::<Vec<_>>();
    for (expression, _) in &arrays {
        assert_eq!(
            members
                .iter()
                .filter(|member| *member == expression)
                .count(),
            1
        );
    }
}

#[test]
fn supplied_array_members_decline_a_non_literal_source() {
    let checked = checked_source(
        r#"
data Packet { samples: [i32; 2]; }
machine build(seed: i32) -> i32 {
    let bare: [i32; 2] = [seed, 3];
    let packet: Packet = Packet { samples: bare };
    seed
}
"#,
        false,
    );
    assert!(
        checked
            .facts
            .values
            .structural_values
            .nodes
            .iter()
            .all(|(_, node)| !matches!(node.kind, CheckedStructuralValueKind::ScalarArray { .. })),
        "a named array source stays with its own place-transfer owner"
    );
}

#[test]
fn supplied_array_member_projected_from_a_borrowed_record_keeps_its_copy_route() {
    let checked = checked_source(
        r#"
data Info { name: [u8; 4]; }
machine clone(info: &Info) -> Info {
    Info { name: info.name }
}
"#,
        false,
    );
    let values = &checked.facts.values.structural_values;
    let member = values
        .record_fields
        .iter()
        .find_map(|(_, field)| match field.value {
            CheckedStructuralRecordFieldValue::Structural(value) => Some(value),
            CheckedStructuralRecordFieldValue::Scalar(_) => None,
        })
        .expect("the projected array member stays a structural record field");
    assert!(
        matches!(
            values.nodes.get(member).kind,
            CheckedStructuralValueKind::CopiedStructuralPlace { .. }
        ),
        "a projected array source copies its place instead of claiming a literal roster"
    );
}
