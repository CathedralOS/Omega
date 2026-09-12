use super::*;
use semantic_vocabulary::{IntegerSign, ObligationId};
use terminal_psi::Operation;

fn array_execution(length: u64) -> TerminalExecution {
    let mut execution = execution();
    execution.structural_types.insert(
        structural_type(4),
        StructuralTypeDeclaration {
            id: structural_type(4),
            identity: "u8".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
    );
    execution
        .structural_types
        .get_mut(&structural_type(2))
        .unwrap()
        .shape = StructuralTypeShape::FixedArray {
        element: structural_type(4),
        length,
    };
    let mut root = parameter(1);
    root.structural_type = structural_type(2);
    execution
        .machines
        .get_mut(&execution.current_machine)
        .unwrap()
        .structural_parameters = vec![root];
    execution
}

fn input(bytes: &[u8]) -> TerminalStructuralByteArrayValue {
    TerminalStructuralByteArrayValue {
        argument_index: 0,
        path: Vec::new(),
        bytes: bytes.to_vec(),
    }
}

#[test]
fn initialized_array_inputs_are_exact_atomic_and_never_implicitly_empty() {
    let mut execution = array_execution(3);
    for bytes in [&[][..], &[1, 2][..], &[1, 2, 3, 4][..]] {
        assert!(execution.bind_byte_arrays(&[input(bytes)]).is_err());
        assert!(execution.structural_byte_array(101, &[]).is_none());
    }
    let valid = input(&[0x11, 0x80, 0xff]);
    let mut absent = valid.clone();
    absent.argument_index = 1;
    assert!(
        execution
            .bind_byte_arrays(&[valid.clone(), absent])
            .is_err()
    );
    assert!(execution.structural_byte_array(101, &[]).is_none());
    assert!(
        execution
            .bind_byte_arrays(&[valid.clone(), valid.clone()])
            .is_err()
    );
    assert!(execution.structural_byte_array(101, &[]).is_none());
    execution
        .bind_byte_arrays(std::slice::from_ref(&valid))
        .unwrap();
    assert!(execution.bind_byte_arrays(&[valid]).is_err());
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[0x11, 0x80, 0xff][..])
    );
    let mut empty = array_execution(0);
    assert!(
        empty
            .prepare_structural_call_arguments(MachineId::new(2).unwrap(), &[argument(1)])
            .is_err()
    );
    assert!(empty.bind_byte_arrays(&[input(&[])]).is_err());
}

#[test]
fn array_view_forwarding_keeps_raw_backing_and_rejects_resize_and_aliases() {
    let mut execution = array_execution(3);
    execution
        .bind_byte_arrays(&[input(&[0x11, 0x80, 0xff])])
        .unwrap();
    for callee in [2, 3] {
        let source = argument(callee - 1);
        let prepared = execution
            .prepare_structural_call_arguments(
                MachineId::new(callee).unwrap(),
                std::slice::from_ref(&source),
            )
            .unwrap();
        execution
            .begin_unit_call(
                MachineId::new(callee).unwrap(),
                &[],
                &[source],
                prepared,
                &[],
                BTreeMap::new(),
            )
            .unwrap();
    }
    assert_eq!(execution.byte_sequence_length(place(3)).unwrap(), 3);
    assert!(
        execution.byte_sequence_values[&place(3)]
            .immutable()
            .is_err()
    );
    assert!(
        execution
            .resolve_boundary_arguments(&[parameter(4)], &[argument(3)])
            .is_err()
    );
    execution
        .machines
        .get_mut(&MachineId::new(2).unwrap())
        .unwrap()
        .structural_parameters = vec![parameter(4), parameter(5)];
    assert!(
        execution
            .prepare_structural_call_arguments(
                MachineId::new(2).unwrap(),
                &[argument(3), argument(3)]
            )
            .is_err()
    );
    let scalar = |bits, value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    let index = ValueId::new(1).unwrap();
    let value = ValueId::new(2).unwrap();
    let length = ValueId::new(3).unwrap();
    execution.values = BTreeMap::from([
        (index, scalar(64, 0)),
        (value, scalar(8, 65)),
        (length, scalar(64, 3)),
    ]);
    let operation = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::ByteSequenceWrite {
            destination: place(3),
            index,
            value,
            length,
            obligation: ObligationId::new(1).unwrap(),
        },
    };
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations = vec![operation];
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[0x11, 0x80, 0xff][..])
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[65, 0x80, 0xff][..])
    );
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[65, 0x80, 0xff][..])
    );
}

#[test]
fn array_input_rejects_wrong_access_path_and_element_type() {
    for mutation in 0..5 {
        let mut execution = array_execution(3);
        let mut supplied = input(&[1, 2, 3]);
        match mutation {
            0 => {
                execution
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow
            }
            1 => supplied.path.push(StructuralPathSegment::FixedIndex(0)),
            2 => {
                execution
                    .structural_types
                    .get_mut(&structural_type(4))
                    .unwrap()
                    .shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                ))
            }
            3 => {
                execution
                    .structural_values
                    .get_mut(&place(1))
                    .unwrap()
                    .structural_type = structural_type(1)
            }
            4 => {
                execution
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .multiplicity = StructuralMultiplicity::Affine
            }
            _ => unreachable!(),
        }
        assert!(execution.bind_byte_arrays(&[supplied]).is_err());
        assert!(execution.structural_byte_arrays.is_empty());
    }
}

#[test]
fn array_field_input_and_presentation_require_record_not_mixed_or_affine() {
    for mixed in [false, true] {
        let mut execution = array_execution(3);
        let fields = vec![StructuralFieldDeclaration {
            id: StructuralFieldId::new(2).unwrap(),
            identity: "raw".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type(2)),
        }];
        execution
            .structural_types
            .get_mut(&structural_type(1))
            .unwrap()
            .shape = if mixed {
            StructuralTypeShape::Mixed {
                fields,
                cases: Vec::new(),
            }
        } else {
            StructuralTypeShape::Record { fields }
        };
        execution
            .structural_values
            .get_mut(&place(1))
            .unwrap()
            .structural_type = structural_type(1);
        execution
            .machines
            .get_mut(&execution.current_machine)
            .unwrap()
            .structural_parameters[0]
            .structural_type = structural_type(1);
        let path = vec![StructuralPathSegment::Field("raw".into())];
        let mut supplied = input(&[1, 128, 255]);
        supplied.path = path.clone();
        let actual = StructuralArgument {
            path: path.clone(),
            ..argument(1)
        };
        if mixed {
            assert!(execution.bind_byte_arrays(&[supplied]).is_err());
            assert!(
                execution
                    .prepare_structural_call_arguments(MachineId::new(2).unwrap(), &[actual])
                    .is_err()
            );
        } else {
            execution.bind_byte_arrays(&[supplied]).unwrap();
            assert_eq!(
                execution.structural_byte_array(101, &path),
                Some(&[1, 128, 255][..])
            );
            assert!(
                execution
                    .prepare_structural_call_arguments(
                        MachineId::new(2).unwrap(),
                        std::slice::from_ref(&actual)
                    )
                    .is_ok()
            );
            execution
                .machines
                .get_mut(&execution.current_machine)
                .unwrap()
                .structural_parameters[0]
                .multiplicity = StructuralMultiplicity::Affine;
            assert!(
                execution
                    .prepare_structural_call_arguments(MachineId::new(2).unwrap(), &[actual])
                    .is_err()
            );
        }
    }
}
