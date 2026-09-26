use crate::effects::AcceptTerminalEffects;
use crate::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
use crate::values::TerminalScalarValue;

use crate::effects::byte_buffers::binding_tests::{
    BTreeMap, BindingRelevance, MachineId, OperationId, OperationKind, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter,
    argument, execution, parameter, place, structural_type,
};
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::ValueId;
use semantic_vocabulary::{IntegerSign, ObligationId};
use terminal_psi::Operation;
use terminal_psi::OperationResult;

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
    std::sync::Arc::get_mut(&mut execution.machines)
        .unwrap()
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
    std::sync::Arc::get_mut(&mut execution.machines)
        .unwrap()
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
        suspension_crossing: None,
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
    std::sync::Arc::get_mut(&mut execution.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap()
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations = vec![operation];
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[0x11, 0x80, 0xff][..])
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_byte_array(101, &[]),
        Some(&[65, 0x80, 0xff][..])
    );
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
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
                std::sync::Arc::get_mut(&mut execution.machines)
                    .unwrap()
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::WriteOnlyBorrow
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
                std::sync::Arc::get_mut(&mut execution.machines)
                    .unwrap()
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .multiplicity = StructuralMultiplicity::Linear
            }
            _ => unreachable!(),
        }
        assert!(execution.bind_byte_arrays(&[supplied]).is_err());
        assert!(execution.structural_byte_arrays.is_empty());
    }
}

#[test]
fn fixed_byte_windows_forward_the_visible_extent_and_write_original_backing() {
    for (start, end) in [(0, 0), (1, 3), (3, 3), (0, 3)] {
        let mut execution = array_execution(3);
        execution
            .bind_byte_arrays(&[input(&[17, 128, 255])])
            .unwrap();
        for callee in [2, 3] {
            let mut source = argument(callee - 1);
            if callee == 2 {
                source
                    .path
                    .push(StructuralPathSegment::FixedByteRange { start, end });
            }
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
        assert_eq!(
            execution.byte_sequence_length(place(3)).unwrap(),
            end - start
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
            (length, scalar(64, (end - start).into())),
        ]);
        let written = execution.execute_byte_sequence_write(&Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::ByteSequenceWrite {
                destination: place(3),
                index,
                value,
                length,
                obligation: ObligationId::new(1).unwrap(),
            },
        });
        let mut expected = [17, 128, 255];
        if start == end {
            assert!(written.is_err());
        } else {
            written.unwrap();
            expected[start as usize] = 65;
        }
        assert_eq!(execution.structural_byte_array(101, &[]).unwrap(), expected);
        assert_eq!(
            execution
                .resume(
                    &mut TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(execution.structural_byte_array(101, &[]).unwrap(), expected);
    }
}

#[test]
fn fixed_byte_windows_check_sibling_aliases_before_binding() {
    let mut execution = array_execution(3);
    execution
        .bind_byte_arrays(&[input(&[17, 128, 255])])
        .unwrap();
    std::sync::Arc::get_mut(&mut execution.machines)
        .unwrap()
        .get_mut(&MachineId::new(2).unwrap())
        .unwrap()
        .structural_parameters = vec![parameter(2), parameter(3)];
    for (left, right, admitted) in [
        ((0, 1), (1, 3), true),
        ((0, 2), (1, 3), false),
        ((1, 1), (1, 3), false),
    ] {
        let arguments = [left, right].map(|(start, end)| {
            let mut source = argument(1);
            source
                .path
                .push(StructuralPathSegment::FixedByteRange { start, end });
            source
        });
        assert_eq!(
            execution
                .prepare_structural_call_arguments(MachineId::new(2).unwrap(), &arguments)
                .is_ok(),
            admitted
        );
        assert_eq!(
            execution.structural_byte_array(101, &[]).unwrap(),
            &[17, 128, 255]
        );
    }
}

#[test]
fn shared_fixed_byte_windows_read_forward_and_reject_mutable_reborrowing() {
    for source_access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for range in [None, Some((0, 0)), Some((1, 3)), Some((3, 3))] {
            let mut execution = array_execution(3);
            let machines = std::sync::Arc::get_mut(&mut execution.machines).unwrap();
            machines
                .get_mut(&MachineId::new(1).unwrap())
                .unwrap()
                .structural_parameters[0]
                .access = source_access;
            for callee in [2, 3] {
                machines
                    .get_mut(&MachineId::new(callee).unwrap())
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            execution
                .bind_byte_arrays(&[input(&[17, 128, 255])])
                .unwrap();
            if source_access == StructuralAccess::SharedBorrow {
                assert!(
                    execution
                        .prepare_array_view_argument(&parameter(2), &argument(1))
                        .is_err()
                );
            }
            let mut source = argument(1);
            source.access = StructuralAccess::SharedBorrow;
            if let Some((start, end)) = range {
                source
                    .path
                    .push(StructuralPathSegment::FixedByteRange { start, end });
            }
            let (start, end) = range.unwrap_or((0, 3));
            let expected = &[17, 128, 255][start as usize..end as usize];
            let mut expected_parameter = parameter(2);
            expected_parameter.access = StructuralAccess::SharedBorrow;
            let boundary = execution
                .resolve_boundary_arguments(&[expected_parameter], std::slice::from_ref(&source))
                .unwrap();
            assert_eq!(boundary.bytes[0].as_deref(), Some(expected));
            assert!(boundary.buffers.is_empty());
            for callee in [2, 3] {
                if callee == 3 {
                    source = argument(2);
                    source.access = StructuralAccess::SharedBorrow;
                }
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
                        &[source.clone()],
                        prepared,
                        &[],
                        BTreeMap::new(),
                    )
                    .unwrap();
            }
            assert_eq!(
                execution.byte_sequence_length(place(3)).unwrap(),
                end - start
            );
            assert_eq!(
                execution.byte_sequence_values[&place(3)]
                    .immutable()
                    .unwrap()
                    .bytes(),
                expected
            );
            assert!(
                execution
                    .prepare_array_view_argument(&parameter(4), &argument(3))
                    .is_err()
            );
            assert!(execution.mutable_byte_sequence_storage(place(3)).is_err());
            if !expected.is_empty() {
                let index = ValueId::new(1).unwrap();
                let length = ValueId::new(2).unwrap();
                let result = ValueId::new(3).unwrap();
                let count_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                execution.values = BTreeMap::from([
                    (
                        index,
                        TerminalScalarValue::Integer {
                            scalar_type: count_type,
                            value: IntegerValue::Unsigned(0),
                        },
                    ),
                    (
                        length,
                        TerminalScalarValue::Integer {
                            scalar_type: count_type,
                            value: IntegerValue::Unsigned((end - start).into()),
                        },
                    ),
                ]);
                execution
                    .execute_byte_sequence_read(&Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: OperationId::new(1).unwrap(),
                        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                            id: result,
                            qualifications: Default::default(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                            ),
                        }),
                        kind: OperationKind::ByteSequenceRead {
                            source: place(3),
                            index,
                            length,
                            obligation: ObligationId::new(1).unwrap(),
                        },
                    })
                    .unwrap();
                assert_eq!(
                    execution.values[&result],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                        value: IntegerValue::Unsigned(expected[0].into()),
                    }
                );
            }
        }
    }
}

#[test]
fn shared_fixed_array_call_after_write_observes_current_backing() {
    let mut execution = array_execution(3);
    execution
        .bind_byte_arrays(&[input(&[17, 128, 255])])
        .unwrap();
    let mut shared_parameter = parameter(3);
    shared_parameter.access = StructuralAccess::SharedBorrow;
    let mut shared_argument = argument(1);
    shared_argument.access = StructuralAccess::SharedBorrow;
    shared_argument
        .path
        .push(StructuralPathSegment::FixedByteRange { start: 1, end: 3 });
    let before = execution
        .resolve_boundary_arguments(
            std::slice::from_ref(&shared_parameter),
            std::slice::from_ref(&shared_argument),
        )
        .unwrap();
    assert_eq!(before.bytes[0].as_deref(), Some(&[128, 255][..]));
    let source = argument(1);
    let prepared = execution
        .prepare_structural_call_arguments(
            MachineId::new(2).unwrap(),
            std::slice::from_ref(&source),
        )
        .unwrap();
    execution
        .begin_unit_call(
            MachineId::new(2).unwrap(),
            &[],
            &[source],
            prepared,
            &[],
            BTreeMap::new(),
        )
        .unwrap();
    let scalar = |bits, value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    let index = ValueId::new(1).unwrap();
    let value = ValueId::new(2).unwrap();
    let length = ValueId::new(3).unwrap();
    execution.values = BTreeMap::from([
        (index, scalar(64, 1)),
        (value, scalar(8, 65)),
        (length, scalar(64, 3)),
    ]);
    execution
        .execute_byte_sequence_write(&Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::ByteSequenceWrite {
                destination: place(2),
                index,
                value,
                length,
                obligation: ObligationId::new(1).unwrap(),
            },
        })
        .unwrap();
    execution
        .resume(
            &mut TerminalFuelMeter::with_allowance(1),
            &mut AcceptTerminalEffects,
        )
        .unwrap();
    assert_eq!(execution.current_machine, MachineId::new(1).unwrap());
    let after = execution
        .resolve_boundary_arguments(&[shared_parameter], &[shared_argument])
        .unwrap();
    assert_eq!(after.bytes[0].as_deref(), Some(&[65, 255][..]));
    assert_eq!(
        execution.structural_byte_array(101, &[]).unwrap(),
        &[17, 65, 255]
    );
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
        std::sync::Arc::get_mut(&mut execution.machines)
            .unwrap()
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
            std::sync::Arc::get_mut(&mut execution.machines)
                .unwrap()
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
