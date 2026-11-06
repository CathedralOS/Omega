//! A saved program with an unknown instruction number must fail verification.
//! These tests put an unassigned operation tag inside an otherwise valid
//! module and check that its decoder reports that exact unknown tag.

use super::super::target_family_tests::unit_machine;
use super::{Writer, encode_operation, operation_tags};
use crate::{CodecError, decode_module, encode_module};
use semantic_vocabulary::{MachineId, OperationId, ScalarQualificationSetId, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule, ValueDeclaration};

/// Keep gaps rejected when the instruction inventory grows. This list covers
/// gaps below 138, including retired tags that must never be reused. Tags after
/// `READ_SEGMENT_SELECTOR` (137) are also unassigned. A new operation takes the
/// next free tag; update this boundary and `wiki/spec/terminal-psi/encoding.md`.
const TAGS_WITHOUT_TABLE_ROWS: [u8; 20] = [
    0, 48, 49, 51, 52, 53, 54, 55, 57, 61, 67, 71, 72, 73, 74, 76, 80, 82, 87, 88,
];

/// One valid module whose only operation is `BooleanConstant`, plus the byte
/// offset of that operation's tag inside the encoded module.
fn module_with_one_operation() -> (TerminalModule, Vec<u8>, usize) {
    let machine_id = MachineId::new(1).unwrap();
    let operation = Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    };
    let mut machine = unit_machine(machine_id);
    machine.blocks[0].operations.push(operation.clone());
    let mut module = TerminalModule::for_entry(machine_id);
    module.machines.push(machine);
    let bytes = encode_module(&module).unwrap();

    let mut writer = Writer::default();
    encode_operation(&mut writer, &operation).unwrap();
    let row = writer.finish();
    let start = bytes
        .windows(row.len())
        .position(|window| window == row.as_slice())
        .expect("the encoded module contains the operation row");
    // The row ends with the tag, its one Boolean field, and the crossing flag.
    let tag_offset = start + row.len() - 3;
    assert_eq!(bytes[tag_offset], operation_tags::BOOLEAN_CONSTANT);
    (module, bytes, tag_offset)
}

#[test]
fn an_operation_tag_without_a_table_row_rejects_the_module() {
    let (module, bytes, tag_offset) = module_with_one_operation();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let unknown = TAGS_WITHOUT_TABLE_ROWS
        .into_iter()
        .chain((operation_tags::READ_SEGMENT_SELECTOR + 1)..=u8::MAX);
    for tag in unknown {
        let mut changed = bytes.clone();
        changed[tag_offset] = tag;
        assert!(
            matches!(
                decode_module(&changed),
                Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
            ),
            "operation tag {tag} has no table row and must reject as an unknown operation tag"
        );
    }
}

#[test]
fn unassigned_operation_tags_71_and_72_reject() {
    let (_, bytes, tag_offset) = module_with_one_operation();
    for tag in [71, 72] {
        let mut changed = bytes.clone();
        changed[tag_offset] = tag;
        assert!(matches!(
            decode_module(&changed),
            Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
        ));
    }
}
