use super::*;
use semantic_vocabulary::{IntegerSign, ObligationId};
use terminal_psi::{Operation, ValueDeclaration};

#[test]
fn ordinary_unit_field_presentation_writes_original_backing_and_rejects_substitution() {
    let mut execution = execution();
    execution
        .structural_byte_sequence_fields
        .insert(field(1), ByteSequenceView::new(vec![10, 20, 30]));
    let actual = StructuralArgument {
        place: place(1),
        path: vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Field("bytes".into()),
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let initialized = execution
        .structural_byte_sequence_fields
        .remove(&field(1))
        .unwrap();
    assert!(
        execution
            .prepare_structural_call_arguments(
                MachineId::new(2).unwrap(),
                std::slice::from_ref(&actual),
            )
            .is_err()
    );
    assert!(
        !execution
            .structural_byte_sequence_fields
            .contains_key(&field(1))
    );
    execution
        .structural_byte_sequence_fields
        .insert(field(1), initialized);
    for mutation in 0..4 {
        let mut hostile = actual.clone();
        match mutation {
            0 => hostile.access = StructuralAccess::SharedBorrow,
            1 => hostile.path[0] = StructuralPathSegment::FixedIndex(2),
            2 => hostile.path[1] = StructuralPathSegment::Field("missing".into()),
            3 => hostile.path.clear(),
            _ => unreachable!(),
        }
        assert!(
            execution
                .prepare_structural_call_arguments(MachineId::new(2).unwrap(), &[hostile])
                .is_err()
        );
    }
    let prepared = execution
        .prepare_structural_call_arguments(
            MachineId::new(2).unwrap(),
            std::slice::from_ref(&actual),
        )
        .unwrap();
    execution
        .begin_unit_call(
            MachineId::new(2).unwrap(),
            &[],
            &[actual],
            prepared,
            &[],
            BTreeMap::new(),
        )
        .unwrap();
    execution.values = BTreeMap::from([
        (scalar(1), unsigned(64, 1)),
        (scalar(2), unsigned(8, 65)),
        (scalar(3), unsigned(64, 3)),
    ]);
    let mut store = write();
    let OperationKind::ByteSequenceWrite { destination, .. } = &mut store.kind else {
        unreachable!()
    };
    *destination = place(2);
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations = vec![store];
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 65, 30]
    );
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(0)].bytes(),
        &[99]
    );
}

#[test]
fn dominated_mutable_block_parameter_supports_fresh_length_and_write() {
    let mut execution = writer();
    let binding_block = BlockId::new(4).unwrap();
    let writing_block = BlockId::new(5).unwrap();
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .terminator = Terminator::Jump {
        edge: EdgeId::new(4).unwrap(),
        target: binding_block,
        arguments: Vec::new(),
        structural_arguments: vec![argument(3)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    execution.blocks.insert(
        binding_block,
        Block {
            id: binding_block,
            parameters: Vec::new(),
            structural_parameters: vec![parameter(4)],
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: EdgeId::new(5).unwrap(),
                target: writing_block,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        },
    );
    let mut store = write();
    let OperationKind::ByteSequenceWrite { destination, .. } = &mut store.kind else {
        unreachable!()
    };
    *destination = place(4);
    execution.values.insert(scalar(3), unsigned(64, 8));
    execution.blocks.insert(
        writing_block,
        Block {
            id: writing_block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: scalar(3),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                    }),
                    kind: OperationKind::ByteSequenceLength { source: place(4) },
                },
                store,
            ],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(6).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
    );
    let mut meter = TerminalFuelMeter::with_allowance(3);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.current, writing_block);
    assert_eq!(execution.values[&scalar(3)], unsigned(64, 3));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 20, 30]
    );
    meter.replenish(4).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 255, 30]
    );
}

#[test]
fn mutable_view_state_transfer_preserves_exact_binding_and_charges_before_commit() {
    let mut execution = writer();
    let destination_block = BlockId::new(4).unwrap();
    let mut store = write();
    let OperationKind::ByteSequenceWrite { destination, .. } = &mut store.kind else {
        unreachable!()
    };
    *destination = place(4);
    execution.blocks.insert(
        destination_block,
        Block {
            id: destination_block,
            parameters: Vec::new(),
            structural_parameters: vec![parameter(4)],
            operations: vec![store],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(5).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
    );
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .terminator = Terminator::Jump {
        edge: EdgeId::new(4).unwrap(),
        target: destination_block,
        arguments: Vec::new(),
        structural_arguments: vec![argument(3)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let referent = execution.structural_values[&place(3)].clone();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert!(!execution.structural_values.contains_key(&place(4)));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values[&place(4)], referent);
    assert_eq!(execution.byte_sequence_length(place(4)).unwrap(), 3);
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 20, 30]
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 255, 30]
    );
    meter.replenish(3).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 255, 30]
    );
}

#[test]
fn mutable_view_state_transfer_rejects_duplicate_loan_and_access_widening() {
    let mut execution = writer();
    let target = BlockId::new(4).unwrap();
    let mut second = parameter(5);
    second.position = 1;
    execution.blocks.insert(
        target,
        Block {
            id: target,
            parameters: Vec::new(),
            structural_parameters: vec![parameter(4), second],
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(4).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
    );
    assert!(
        execution
            .prepare_block_bindings(target, &[], &[argument(3), argument(3)])
            .is_err()
    );
    execution
        .blocks
        .get_mut(&target)
        .unwrap()
        .structural_parameters
        .pop();
    assert!(
        execution
            .prepare_block_bindings(target, &[], &[argument(3)])
            .is_ok()
    );
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::Owned,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        execution
            .blocks
            .get_mut(&target)
            .unwrap()
            .structural_parameters[0]
            .access = access;
        let mut argument = argument(3);
        argument.access = access;
        assert!(
            execution
                .prepare_block_bindings(target, &[], &[argument])
                .is_err()
        );
    }
    assert!(!execution.structural_values.contains_key(&place(4)));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 20, 30]
    );
}

fn scalar(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}
fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}
fn write() -> Operation {
    Operation {
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::ByteSequenceWrite {
            destination: place(3),
            index: scalar(1),
            value: scalar(2),
            length: scalar(3),
            obligation: ObligationId::new(1).unwrap(),
        },
    }
}
fn writer() -> TerminalExecution {
    let mut execution = execution();
    execution
        .structural_byte_sequence_fields
        .insert(field(1), ByteSequenceView::new(vec![10, 20, 30]));
    enter_provider(&mut execution);
    let prepared = execution
        .prepare_structural_call_arguments(MachineId::new(3).unwrap(), &[argument(2)])
        .unwrap();
    execution
        .begin_unit_call(
            MachineId::new(3).unwrap(),
            &[],
            &[argument(2)],
            prepared,
            &[],
            BTreeMap::new(),
        )
        .unwrap();
    execution.values = BTreeMap::from([
        (scalar(1), unsigned(64, 1)),
        (scalar(2), unsigned(8, 255)),
        (scalar(3), unsigned(64, 3)),
    ]);
    execution
}

#[test]
fn fixed_view_write_survives_suspension_nested_return_and_preserves_immutable_tail() {
    let mut execution = writer();
    let immutable = execution.structural_byte_sequence_fields[&field(1)].clone();
    execution
        .byte_sequence_values
        .insert(place(90), ByteSequenceBinding::Immutable(immutable.clone()));
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations = vec![
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: scalar(3),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceLength { source: place(3) },
        },
        write(),
    ];
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 20, 30]
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.values[&scalar(3)], unsigned(64, 3));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 20, 30]
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[10, 255, 30]
    );
    assert_eq!(immutable.bytes(), &[10, 20, 30]);
    assert_eq!(
        execution.byte_sequence_values[&place(90)]
            .immutable()
            .unwrap()
            .bytes(),
        &[10, 20, 30]
    );
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(0)].bytes(),
        &[99]
    );
    for _ in 0..2 {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(
            execution.structural_byte_sequence_fields[&field(1)].bytes(),
            &[10, 255, 30]
        );
    }
    meter.replenish(3).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_sequence_field(
            101,
            &[StructuralPathSegment::FixedIndex(1)],
            field(1).field
        ),
        Some(&[10, 255, 30][..])
    );
}

#[test]
fn fixed_view_write_rejects_bad_operands_binding_and_stale_extent_without_mutation() {
    for mutation in 0..11 {
        let mut execution = writer();
        match mutation {
            0 => {
                execution.values.insert(scalar(1), unsigned(64, 3));
            }
            1 => {
                execution.values.insert(scalar(3), unsigned(64, 8));
            }
            2 => {
                execution.values.insert(scalar(2), unsigned(8, 256));
            }
            3 => {
                execution.values.insert(scalar(2), unsigned(64, 1));
            }
            4 => {
                execution.byte_sequence_values.remove(&place(3));
            }
            5 => {
                execution.byte_sequence_values.insert(
                    place(3),
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(vec![10, 20, 30])),
                );
            }
            6 => {
                execution
                    .structural_values
                    .get_mut(&place(3))
                    .unwrap()
                    .opaque_identity += 1;
            }
            7 => {
                execution
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            8 => {
                execution.values.remove(&scalar(1));
            }
            9 => {
                execution.values.insert(scalar(1), unsigned(8, 1));
            }
            10 => {
                execution
                    .structural_byte_sequence_fields
                    .insert(field(1), ByteSequenceView::new(vec![10, 20]));
            }
            _ => unreachable!(),
        }
        let before = execution.structural_byte_sequence_fields[&field(1)]
            .bytes()
            .to_vec();
        assert!(
            execution.execute_byte_sequence_write(&write()).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(
            execution.structural_byte_sequence_fields[&field(1)].bytes(),
            before
        );
    }
}

#[test]
fn fixed_view_length_uses_live_extent_and_mutable_write_never_changes_it() {
    let mut execution = writer();
    assert_eq!(execution.byte_sequence_length(place(3)).unwrap(), 3);
    execution.execute_byte_sequence_write(&write()).unwrap();
    execution.values.insert(scalar(1), unsigned(64, 0));
    execution.values.insert(scalar(2), unsigned(8, 0));
    execution.execute_byte_sequence_write(&write()).unwrap();
    assert_eq!(execution.byte_sequence_length(place(3)).unwrap(), 3);
    assert_eq!(
        execution.structural_byte_sequence_fields[&field(1)].bytes(),
        &[0, 255, 30]
    );
    execution
        .structural_values
        .get_mut(&place(3))
        .unwrap()
        .path
        .clear();
    assert!(execution.byte_sequence_length(place(3)).is_err());
}
