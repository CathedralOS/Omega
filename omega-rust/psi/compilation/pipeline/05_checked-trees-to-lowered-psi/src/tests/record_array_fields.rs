//! A record literal that supplies a closed primitive array member lowers that
//! member as one `EstablishScalarArray` child whose element roster is the
//! authored leaves in row-major order, and the enclosing `EstablishRecord`
//! consumes the completed array as an ordinary structural field.
use crate::{TerminalMachineSelection, lower_machine};
use semantic_vocabulary::{IntegerValue, ValueId};
use terminal_psi::{Block, OperationKind, RecordFieldValue, Terminator};

const SOURCE: &str = "
    data Pair { left: [i32; 3]; right: [i32; 2]; }
    data Grid { cells: [[i16; 2]; 2]; tag: u8; }
    data Framed { pair: Pair; grid: Grid; }
    machine build(seed: i32) -> Framed {
        Framed {
            pair: Pair { left: [seed, 0 - 2, 7], right: [5, seed] },
            grid: Grid { cells: [[1, 2], [3, 0 - 4]], tag: 9 }
        }
    }
";

/// One array leaf as the lowered producer names it: a folded or computed
/// integer, or the machine's scalar input.
#[derive(Debug, PartialEq, Eq)]
enum Leaf {
    Integer(i128),
    Input,
}

/// Follow a value to its producer. Retained leaf evaluation may complete a
/// value through a private block, so a block parameter resolves through the
/// one jump that binds it.
fn leaf(blocks: &[&Block], value: ValueId) -> Leaf {
    let operations = blocks.iter().flat_map(|block| &block.operations);
    if let Some(producer) = operations.clone().find(|operation| {
        operation
            .result
            .scalar()
            .is_some_and(|result| result.id == value)
    }) {
        let integer = |value: ValueId| match leaf(blocks, value) {
            Leaf::Integer(value) => value,
            Leaf::Input => panic!("a folded operand reads the machine input"),
        };
        return Leaf::Integer(match &producer.kind {
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(value),
            } => *value,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value),
            } => i128::try_from(*value).expect("array leaf fits i128"),
            OperationKind::ExactIntegerSubtract { left, right, .. } => {
                integer(*left) - integer(*right)
            }
            other => panic!("unexpected array leaf producer {other:?}"),
        });
    }
    let Some((target, position)) = blocks.iter().find_map(|block| {
        block
            .parameters
            .iter()
            .position(|parameter| parameter.id == value)
            .map(|position| (block.id, position))
    }) else {
        return Leaf::Input;
    };
    let mut bindings = blocks.iter().filter_map(|block| match &block.terminator {
        Terminator::Jump {
            target: jumped,
            arguments,
            ..
        } if *jumped == target => Some(arguments[position]),
        _ => None,
    });
    match (bindings.next(), bindings.next()) {
        (Some(bound), None) => leaf(blocks, bound),
        _ => Leaf::Input,
    }
}

#[test]
fn supplied_array_members_establish_ordered_scalar_array_children() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("build"))
        .expect("record literals with supplied array members lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("supplied array members verify independently");
    let blocks = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .collect::<Vec<_>>();
    let operations = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let arrays = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishScalarArray { elements } => Some((
                operation.result.structural().expect("array place").place,
                elements
                    .iter()
                    .map(|element| leaf(&blocks, *element))
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    // Two arrays in one record literal keep their own rosters; the nested
    // `[[i16; 2]; 2]` member flattens outer index first; `0 - 2` and `0 - 4`
    // keep their negative values; the runtime input stays in its slot.
    assert_eq!(
        arrays
            .iter()
            .map(|(_, leaves)| leaves.as_slice())
            .collect::<Vec<_>>(),
        [
            &[Leaf::Input, Leaf::Integer(-2), Leaf::Integer(7)][..],
            &[Leaf::Integer(5), Leaf::Input][..],
            &[
                Leaf::Integer(1),
                Leaf::Integer(2),
                Leaf::Integer(3),
                Leaf::Integer(-4)
            ][..],
        ]
    );
    // Each array place is consumed as a structural record field, never as a
    // scalar operand or a second establishment.
    let consumed = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishRecord { fields } => Some(fields),
            _ => None,
        })
        .flatten()
        .filter_map(|field| match &field.value {
            RecordFieldValue::Structural(argument) => Some(argument.place),
            RecordFieldValue::Scalar { .. } => None,
        })
        .collect::<Vec<_>>();
    for (place, _) in &arrays {
        assert_eq!(
            consumed
                .iter()
                .filter(|candidate| *candidate == place)
                .count(),
            1,
            "each supplied array joins exactly one record field"
        );
    }
}

#[test]
fn supplied_array_member_rejects_a_substituted_leaf_computation() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let plans = &mut checked.facts.values.structural_values;
    let (right, left) = {
        let mut arrays = plans.nodes.iter().filter_map(|(handle, node)| {
            matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            )
            .then_some(handle)
        });
        let left = arrays.next().expect("left array");
        let right = arrays.next().expect("right array");
        (right, left)
    };
    let checked_trees::CheckedStructuralValueKind::ScalarArray { elements } =
        plans.nodes.get(left).kind.clone()
    else {
        unreachable!()
    };
    let checked_trees::CheckedStructuralValueKind::ScalarArray { elements: swapped } =
        &mut plans.nodes.get_mut(right).kind
    else {
        unreachable!()
    };
    // The right array's first leaf now names the left array's first
    // computation; replay must rejoin each leaf to its own authored operand.
    swapped[0] = elements[0];
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("build")).is_err(),
        "a leaf computation borrowed from another array occurrence must reject"
    );
}
