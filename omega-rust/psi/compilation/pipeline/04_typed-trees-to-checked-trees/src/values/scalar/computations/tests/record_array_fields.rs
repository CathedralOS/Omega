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
fn supplied_named_array_members_copy_local_and_parameter_storage() {
    let checked = checked_source(
        r#"
data Packet { samples: [i32; 2]; }
machine build(seed: i32) -> i32 {
    let mut bare: [i32; 2] = [seed, 3];
    let packet: Packet = Packet { samples: bare };
    let second: Packet = Packet { samples: bare };
    seed
}
machine wrap(samples: [i32; 2]) -> Packet { Packet { samples: samples } }
"#,
        false,
    );
    let values = &checked.facts.values.structural_values;
    let copies = values
        .record_fields
        .iter()
        .map(|(_, field)| {
            let CheckedStructuralRecordFieldValue::Structural(value) = field.value else {
                panic!("the array is a structural operand");
            };
            let CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
                &values.nodes.get(value).kind
            else {
                panic!("each array read snapshots its existing storage");
            };
            assert!(source.path.is_empty(), "whole-array read");
            (value, &source.source)
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 3);
    assert_ne!(copies[0].0, copies[1].0, "reads have independent snapshots");
    assert_eq!(
        copies[0].1, copies[1].1,
        "reads retain the same source storage"
    );
    // Parameters use the same copied-place operation as locals, and since
    // `cd54a8d27dd` they use the same source plan too: `StructuralLocal` names
    // a named formal as well as a live local, because value planning precedes
    // the final structural parameter roster. The variant alone therefore no
    // longer tells the two apart, so the discrimination is the symbol --
    // `wrap`'s copy names its own `samples`, not `build`'s `bare`.
    let (
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local },
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: formal },
    ) = (copies[0].1, copies[2].1)
    else {
        panic!(
            "a local and a named formal both copy from their authored symbol: {:?}, {:?}",
            copies[0].1, copies[2].1
        );
    };
    assert_ne!(
        local, formal,
        "the parameter copy names its own formal, not the other machine's local"
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

#[test]
fn array_assignment_retains_the_destination_arithmetic_policy() {
    for array_type in ["[i32; 3] in Wrapping", "[[u16; 2]; 4] in Saturating"] {
        let source = r#"
data Buffers {
    source: ARRAY;
    destination: ARRAY;
}
machine copy(buffers: &mut Buffers) {
    buffers.destination = buffers.source;
}
"#
        .replace("ARRAY", array_type);
        let checked = checked_source(&source, false);
        let machine = &checked.machines()[0];
        let state = &checked.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::Assignment(assignment) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("expected the authored array assignment");
        };
        let expected =
            validation::declared_place_type_raw(&checked, machine, Some(state), assignment.target)
                .expect("destination retains its declared array type");
        let values = &checked.facts.values.structural_values;
        let root = values
            .root_for_expression(state.symbol, 0, assignment.value)
            .expect("an array assignment plans the same copied value as a field initializer");
        assert_eq!(
            checked.normalized_type_identity(root.type_reference),
            checked.normalized_type_identity(expected),
            "structural value custody must not erase the destination's arithmetic policy"
        );
        let CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf } =
            &values.nodes.get(root.root).kind
        else {
            panic!("whole-array assignment must copy its exact source place");
        };
        assert_eq!(
            checked.normalized_type_identity(*leaf),
            checked.normalized_type_identity(expected),
            "the source leaf retains its complete semantic type"
        );
        let carrier = validation::peel_arithmetic_policy(&checked, expected);
        assert_eq!(
            source.type_identity,
            checked.normalized_type_identity(carrier).as_str(),
            "the copy's physical source names the registered storage carrier"
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|declaration| declaration.identity == source.type_identity),
            "the copy must rejoin an actual carrier declaration"
        );
    }
}
