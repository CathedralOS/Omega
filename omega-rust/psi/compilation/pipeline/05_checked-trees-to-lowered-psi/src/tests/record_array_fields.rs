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

#[test]
fn shared_view_members_construct_before_indexed_replacement() {
    for body in [
        "machine fill(context: &mut Context) {
            context.header = \"heading\";
            context.cells[1] = Cell { label: \"hello\", value: 7 };
        }",
        "machine fill(text: &[u8] in Utf8) {
            let cell: Cell = Cell { label: text, value: 7 };
            let value: i32 = cell.value;
        }",
    ] {
        let source = format!(
            "domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
             data Cell {{ label: &[u8] in Utf8; value: i32; }}
             data Context {{ header: &[u8] in Utf8; cells: [Cell; 2]; }}
             {body}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
            .unwrap_or_else(|error| panic!("{body}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("descriptor custody and predicate implication verify");
        let encoded = terminal_codec::encode_module(&lowered.semantic_module)
            .expect("qualified descriptor members have ordinary wire formation");
        let decoded =
            terminal_codec::decode_module(&encoded).expect("descriptor construction round trips");
        assert_eq!(decoded, lowered.semantic_module);
        if body.contains("\"hello\"") {
            let descriptor = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .find_map(|operation| match &operation.kind {
                    OperationKind::EstablishRecord { fields } => {
                        fields.iter().find_map(|field| match &field.value {
                            RecordFieldValue::Structural(argument) => Some(argument.place),
                            _ => None,
                        })
                    }
                    _ => None,
                })
                .expect("record descriptor operand");
            let mut unavailable = lowered.semantic_module.clone();
            let block = unavailable
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .find(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::EstablishByteSequenceLiteral { destination, .. }
                                if destination == descriptor
                        )
                    })
                })
                .expect("literal's defining block");
            let position = block
                .operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::EstablishByteSequenceLiteral { destination, .. }
                            if destination == descriptor
                    )
                })
                .expect("literal's definition");
            let literal = block.operations.remove(position);
            let OperationKind::EstablishByteSequenceLiteral { destination, .. } = literal.kind
            else {
                unreachable!("selected literal establishment");
            };
            let consumer = unavailable
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .find(|block| {
                    block
                        .operations
                        .iter()
                        .any(|operation| match &operation.kind {
                            OperationKind::EstablishRecord { fields } => {
                                fields.iter().any(|field| {
                                    matches!(&field.value, RecordFieldValue::Structural(argument)
                            if argument.place == destination)
                                })
                            }
                            _ => false,
                        })
                })
                .expect("record consumes the descriptor");
            consumer.operations.push(literal);
            assert!(
                terminal_verifier::validate_module(&unavailable).is_err(),
                "unrestricted operands must still be defined before use"
            );

            let mut wrong_access = lowered.semantic_module.clone();
            let argument = wrong_access
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match &mut operation.kind {
                    OperationKind::EstablishRecord { fields } => {
                        fields.iter_mut().find_map(|field| match &mut field.value {
                            RecordFieldValue::Structural(argument) => Some(argument),
                            _ => None,
                        })
                    }
                    _ => None,
                })
                .expect("descriptor record operand");
            argument.access = terminal_psi::StructuralAccess::MutableBorrow;
            assert!(
                terminal_verifier::validate_module(&wrong_access).is_err(),
                "copying a shared descriptor cannot grant mutable authority"
            );
            assert!(
                terminal_codec::encode_module(&wrong_access).is_err(),
                "wire formation also rejects changed operand custody"
            );

            let mut corrupted = lowered.semantic_module.clone();
            let literal = corrupted
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match &mut operation.kind {
                    OperationKind::EstablishByteSequenceLiteral {
                        destination, bytes, ..
                    } if *destination == descriptor => Some(bytes),
                    _ => None,
                })
                .expect("literal descriptor operand");
            *literal = vec![255];
            assert!(
                terminal_verifier::validate_module(&corrupted).is_err(),
                "the receiving field check rejects invalid UTF-8 before proof-envelope identity checks"
            );
            assert!(
                terminal_verifier::verify_module(
                    &corrupted,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default(),
                )
                .is_err(),
                "the field declaration cannot prove corrupted UTF-8 contents"
            );
        }
    }
}

#[test]
fn a_record_cannot_export_call_scoped_descriptor_backing() {
    let rejected = crate::front_end::checked_program_result(
        "domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
         data Cell { label: &[u8] in Utf8; value: i32; }
         data Context { cells: [Cell; 2]; }
         machine fill(context: &mut Context, text: &[u8] in Utf8) {
             context.cells[1] = Cell { label: text, value: 7 };
         }",
    )
    .expect_err("a borrowed parameter cannot escape into caller storage");
    assert!(
        rejected
            .iter()
            .any(|diagnostic| diagnostic.message.contains("normal-exit backing relation"))
    );
}

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
