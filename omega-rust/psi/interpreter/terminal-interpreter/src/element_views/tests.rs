use super::*;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralTypeId;

fn collection() -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity: 991,
        structural_type: StructuralTypeId::new(1).unwrap(),
        qualifications: Vec::new(),
        path: vec![StructuralPathSegment::Field("rooms".into())],
    }
}

#[test]
fn structural_element_view_keeps_storage_identity_across_descriptor_copies() {
    let backing = collection();
    let view = ElementView::structural(backing.clone(), 3, StructuralAccess::MutableBorrow);
    let transferred = view.clone();
    let path = [
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("nested".into()),
    ];
    let (retained, selected) = transferred.structural_projection(&path, true).unwrap();
    assert_eq!(retained, backing);
    assert_eq!(selected, path);
    assert_eq!(view.len(), 3);
    assert!(
        view.get(1).is_none(),
        "structural storage is not a scalar copy"
    );
}

#[test]
fn structural_element_view_window_preserves_offsets_and_attenuates_access() {
    let backing = collection();
    let view = ElementView::structural(backing.clone(), 4, StructuralAccess::MutableBorrow);
    let window = view
        .subslice(1, 4, StructuralAccess::SharedBorrow)
        .unwrap()
        .subslice(1, 3, StructuralAccess::SharedBorrow)
        .unwrap();
    let (retained, path) = window
        .structural_projection(&[StructuralPathSegment::FixedIndex(1)], false)
        .unwrap();
    assert_eq!(retained, backing);
    assert_eq!(path, [StructuralPathSegment::FixedIndex(3)]);
    assert!(
        window
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], true)
            .is_err()
    );
    assert!(
        window
            .structural_projection(&[StructuralPathSegment::FixedIndex(2)], false)
            .is_err()
    );
    assert!(window.structural_projection(&[], false).is_err());
    assert!(
        window
            .subslice(0, 3, StructuralAccess::SharedBorrow)
            .is_none()
    );
    assert!(
        window
            .subslice(2, 1, StructuralAccess::SharedBorrow)
            .is_none()
    );
    let empty = window
        .subslice(2, 2, StructuralAccess::SharedBorrow)
        .unwrap();
    assert_eq!(empty.len(), 0);
    assert!(
        empty
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], false)
            .is_err()
    );
}

fn runtime_subslice() -> (TerminalExecution, Operation) {
    use crate::execution::ExecutableMachine;
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, ObligationId, OperationId};
    use terminal_psi::{
        Block, StructuralOperationResult, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalMachineResult, Terminator,
    };
    let machine_id = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let view_type = StructuralTypeId::new(2).unwrap();
    let source = PlaceId::new(1).unwrap();
    let destination = PlaceId::new(2).unwrap();
    let operation = Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: destination,
            structural_type: view_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ElementViewSubslice {
            source,
            start: ValueId::new(1).unwrap(),
            end: ValueId::new(2).unwrap(),
            length: ValueId::new(3).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        },
    };
    let parameter = StructuralParameterDeclaration {
        place: source,
        position: 0,
        is_self: false,
        structural_type: view_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let machine = ExecutableMachine {
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: destination,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation.id,
                    structural_type: view_type,
                },
            },
        ],
        entry_claims: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Unit,
        entry: block,
        blocks: BTreeMap::from([(
            block,
            Block {
                id: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: vec![operation.clone()],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        )]),
    };
    let types = [
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Record".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: view_type,
            identity: "View".into(),
            shape: StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(1).unwrap(),
            },
        },
    ];
    // Private source-free execution state exercises defensive operation checks;
    // verifier admission of the same authority is covered by Terminal controls.
    let program = crate::program::AdmittedTerminalProgram::for_test(
        BTreeMap::from([(machine_id, machine)]),
        types.into_iter().map(|row| (row.id, row)).collect(),
    );
    let input = TerminalStructuralValue {
        opaque_identity: 100,
        structural_type: view_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_with_program(
        Arc::new(program),
        &[],
        &[input],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    execution.element_view_values.insert(
        source,
        ElementView::structural(collection(), 4, StructuralAccess::MutableBorrow),
    );
    for (number, count) in [(1, 1), (2, 3), (3, 4)] {
        execution.values.insert(
            ValueId::new(number).unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(count),
            },
        );
    }
    (execution, operation)
}

#[test]
fn element_view_call_binding_preserves_and_attenuates_exact_access() {
    let (mut execution, _) = runtime_subslice();
    let source = PlaceId::new(1).unwrap();
    let mut parameter =
        execution.program.machines[&execution.current_machine].structural_parameters[0].clone();
    let mut argument = terminal_psi::StructuralArgument {
        place: source,
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    };
    let resolved = execution.structural_values[&source].clone();
    let bind = |execution: &TerminalExecution,
                parameter: &terminal_psi::StructuralParameterDeclaration,
                argument: &terminal_psi::StructuralArgument| {
        execution.bind_element_view_arguments(
            std::slice::from_ref(parameter),
            std::slice::from_ref(argument),
            std::slice::from_ref(&resolved),
        )
    };
    let mutable = bind(&execution, &parameter, &argument).unwrap();
    assert_eq!(mutable[&source].access, StructuralAccess::MutableBorrow);
    parameter.access = StructuralAccess::SharedBorrow;
    assert!(bind(&execution, &parameter, &argument).is_err());
    argument.access = StructuralAccess::SharedBorrow;
    let shared = bind(&execution, &parameter, &argument).unwrap();
    assert_eq!(shared[&source].access, StructuralAccess::SharedBorrow);
    assert!(
        shared[&source]
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], true)
            .is_err()
    );
    assert_eq!(
        execution.element_view_values[&source].access,
        StructuralAccess::MutableBorrow
    );
    execution
        .element_view_values
        .insert(source, shared[&source].clone());
    parameter.access = StructuralAccess::MutableBorrow;
    argument.access = StructuralAccess::MutableBorrow;
    assert!(
        bind(&execution, &parameter, &argument).is_err(),
        "a shared received descriptor cannot regain exclusive access"
    );
}

#[test]
fn element_view_scalar_home_pins_exact_frame_despite_place_collisions() {
    let (mut execution, _) = scalar_home_subslice();
    let source = PlaceId::new(1).unwrap();
    let backing = PlaceId::new(9).unwrap();
    let home = execution.element_view_values[&source].home.clone().unwrap();
    let caller = execution.current_machine;
    let callee = semantic_vocabulary::MachineId::new(2).unwrap();
    let mut body = execution.program.machines[&caller].clone();
    body.structural_parameters.truncate(1);
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .machines
        .insert(callee, body);
    let arguments = [terminal_psi::StructuralArgument {
        place: source,
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    }];
    let prepared = execution
        .prepare_structural_call_arguments(callee, &arguments)
        .unwrap();
    execution
        .begin_unit_call(callee, &[], &arguments, prepared, &[], BTreeMap::new())
        .unwrap();
    let array_type = execution.call_stack[0].scalar_array_values[&backing].structural_type;
    execution.scalar_array_values.insert(
        backing,
        crate::scalar_array::TerminalScalarArrayValue {
            structural_type: array_type,
            elements: vec![TerminalScalarValue::Boolean(false); 4],
        },
    );
    let path = [StructuralPathSegment::FixedIndex(1)];
    execution
        .store_primitive_home(home.storage, &path, TerminalScalarValue::Boolean(true))
        .unwrap();
    assert_eq!(
        execution.call_stack[0].scalar_array_values[&backing].elements[1],
        TerminalScalarValue::Boolean(true)
    );
    assert_eq!(
        execution.scalar_array_values[&backing].elements[1],
        TerminalScalarValue::Boolean(false),
        "callee's equal PlaceId is a different activation"
    );
    assert_eq!(
        execution
            .read_primitive_home(home.storage, &path, ScalarType::Boolean)
            .unwrap(),
        TerminalScalarValue::Boolean(true)
    );
    let retired = execution.capture_primitive_home(backing).unwrap();
    execution.call_stack.pop();
    assert!(
        execution
            .read_primitive_home(retired, &path, ScalarType::Boolean)
            .is_err()
    );
    assert!(
        execution
            .read_primitive_home(home.storage, &path, ScalarType::Boolean)
            .is_err(),
        "the replacement current machine cannot impersonate the captured caller"
    );
}

#[test]
fn element_view_state_binding_preserves_window_access_and_affine_source_debt() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let (mut execution, operation) = runtime_subslice();
        execution.execute_element_view_subslice(&operation).unwrap();
        let result = operation.result.structural().unwrap();
        let target = semantic_vocabulary::BlockId::new(2).unwrap();
        let destination = PlaceId::new(3).unwrap();
        let parameter = terminal_psi::StructuralParameterDeclaration {
            place: destination,
            position: 0,
            is_self: false,
            structural_type: result.structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let program = Arc::get_mut(&mut execution.program).unwrap();
        let machine = program
            .machines
            .get_mut(&execution.current_machine)
            .unwrap();
        let mut block = machine.blocks[&machine.entry].clone();
        block.id = target;
        block.operations.clear();
        block.structural_parameters = vec![parameter];
        machine.blocks.insert(target, block);
        machine
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: destination,
                kind: StructuralPlaceKind::BlockParameter {
                    block: target,
                    position: 0,
                },
            });
        let argument = terminal_psi::StructuralArgument {
            place: result.place,
            path: Vec::new(),
            access,
        };
        let debt = execution.live_affine_frontier.clone();
        let binding = execution
            .prepare_block_bindings(target, &[], std::slice::from_ref(&argument), true)
            .unwrap();
        binding.validate_discards(&execution, &[], &[]).unwrap();
        binding.commit(&mut execution);
        assert_eq!(
            execution.live_affine_frontier, debt,
            "a borrowed state argument cannot consume its descriptor owner's cleanup"
        );
        assert!(execution.element_view_values.contains_key(&result.place));
        let received = &execution.element_view_values[&destination];
        assert_eq!(received.len(), 2);
        assert_eq!(received.access, access);
        let (backing, path) = received
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], false)
            .unwrap();
        assert_eq!(backing, collection());
        assert_eq!(path, [StructuralPathSegment::FixedIndex(1)]);
        if access == StructuralAccess::MutableBorrow {
            let shared = execution.element_view_values[&result.place]
                .subslice(0, 2, StructuralAccess::SharedBorrow)
                .unwrap();
            execution.element_view_values.insert(result.place, shared);
            assert!(
                execution
                    .prepare_block_bindings(target, &[], &[argument], true)
                    .is_err(),
                "state transfer cannot promote a shared descriptor"
            );
        }
    }
}

#[test]
fn terminal_mutable_element_subslice_preserves_backing_and_exact_frontier() {
    let (mut execution, operation) = runtime_subslice();
    execution.execute_element_view_subslice(&operation).unwrap();
    let result = operation.result.structural().unwrap();
    let child = &execution.element_view_values[&result.place];
    let (backing, path) = child
        .structural_projection(&[StructuralPathSegment::FixedIndex(1)], true)
        .unwrap();
    assert_eq!(backing, collection());
    assert_eq!(path, [StructuralPathSegment::FixedIndex(2)]);
    assert_eq!(
        execution
            .live_affine_frontier
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![terminal_psi::StructuralAffineDiscard {
            place: result.place,
            structural_type: result.structural_type,
            path: Vec::new()
        }]
    );
    assert!(
        execution.execute_element_view_subslice(&operation).is_err(),
        "a live affine child cannot be silently rebound"
    );

    let (mut execution, mut shared) = runtime_subslice();
    let OperationResult::Structural(result) = &mut shared.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    execution.execute_element_view_subslice(&shared).unwrap();
    assert!(
        execution.element_view_values[&PlaceId::new(2).unwrap()]
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], true)
            .is_err()
    );
    assert!(execution.live_affine_frontier.is_empty());
}

#[test]
fn terminal_mutable_element_subslice_rejects_forged_permission_and_metadata_atomically() {
    for corruption in 0..6 {
        let (mut execution, mut operation) = runtime_subslice();
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!()
        };
        match corruption {
            0 => {
                execution
                    .element_view_values
                    .get_mut(&PlaceId::new(1).unwrap())
                    .unwrap()
                    .access = StructuralAccess::SharedBorrow
            }
            1 => {
                Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .machines
                    .values_mut()
                    .next()
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow
            }
            2 => operation.id = semantic_vocabulary::OperationId::new(2).unwrap(),
            3 => result.multiplicity = StructuralMultiplicity::Linear,
            4 => result.structural_type = StructuralTypeId::new(1).unwrap(),
            5 => {
                execution.values.insert(
                    ValueId::new(3).unwrap(),
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        value: IntegerValue::Unsigned(5),
                    },
                );
            }
            _ => unreachable!(),
        }
        assert!(
            execution.execute_element_view_subslice(&operation).is_err(),
            "corruption {corruption}"
        );
        assert!(
            !execution
                .element_view_values
                .contains_key(&PlaceId::new(2).unwrap())
        );
        assert!(
            !execution
                .structural_values
                .contains_key(&PlaceId::new(2).unwrap())
        );
        assert!(execution.live_affine_frontier.is_empty());
    }
}

fn scalar_home_subslice() -> (TerminalExecution, Operation) {
    let (mut execution, operation) = runtime_subslice();
    let source = PlaceId::new(1).unwrap();
    let backing = PlaceId::new(9).unwrap();
    let element_type = StructuralTypeId::new(1).unwrap();
    let array_type = StructuralTypeId::new(3).unwrap();
    let program = Arc::get_mut(&mut execution.program).unwrap();
    program
        .structural_types
        .get_mut(&element_type)
        .unwrap()
        .shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    program.structural_types.insert(
        array_type,
        terminal_psi::StructuralTypeDeclaration {
            id: array_type,
            identity: "Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 4,
            },
        },
    );
    let machine = program.machines.values_mut().next().unwrap();
    machine
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: backing,
            position: 1,
            is_self: false,
            structural_type: array_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: backing,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let elements = vec![TerminalScalarValue::Boolean(false); 4];
    execution.scalar_array_values.insert(
        backing,
        crate::scalar_array::TerminalScalarArrayValue {
            structural_type: array_type,
            elements: elements.clone(),
        },
    );
    execution.element_view_values.insert(
        source,
        ElementView::new_mutable(
            elements,
            ElementViewHome {
                storage: execution.capture_primitive_home(backing).unwrap(),
                path: Vec::new(),
            },
        ),
    );
    (execution, operation)
}

#[test]
fn terminal_mutable_element_subslice_writes_original_scalar_home_and_restored_parent_reads() {
    let (mut execution, operation) = scalar_home_subslice();
    let source = PlaceId::new(1).unwrap();
    let child = PlaceId::new(2).unwrap();
    let backing = PlaceId::new(9).unwrap();
    execution.execute_element_view_subslice(&operation).unwrap();
    let value = ValueId::new(4).unwrap();
    let index = ValueId::new(5).unwrap();
    let length = ValueId::new(6).unwrap();
    execution
        .values
        .insert(value, TerminalScalarValue::Boolean(true));
    for (identity, count) in [(index, 0), (length, 2)] {
        execution.values.insert(
            identity,
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(count),
            },
        );
    }
    let write = Operation {
        id: semantic_vocabulary::OperationId::new(2).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::ElementViewWrite {
            destination: child,
            index,
            value,
            length,
            obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
        },
    };
    execution.execute_element_view_write(&write).unwrap();
    assert_eq!(
        execution.scalar_array_values[&backing].elements,
        vec![
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(false)
        ]
    );
    // Model the already-verified child release; the runtime must observe the
    // original home rather than the parent's pre-child snapshot afterwards.
    execution.element_view_values.remove(&child);
    execution.structural_values.remove(&child);
    execution.live_affine_frontier.clear();
    let observed = ValueId::new(7).unwrap();
    let read = Operation {
        id: semantic_vocabulary::OperationId::new(3).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            id: observed,
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::ElementViewRead {
            source,
            index: ValueId::new(1).unwrap(),
            length: ValueId::new(3).unwrap(),
            path: Vec::new(),
            obligation: semantic_vocabulary::ObligationId::new(3).unwrap(),
        },
    };
    execution.execute_element_view_read(&read).unwrap();
    assert_eq!(
        execution.values[&observed],
        TerminalScalarValue::Boolean(true)
    );
    let shared = execution.element_view_values[&source]
        .subslice(0, 4, StructuralAccess::SharedBorrow)
        .unwrap();
    assert!(
        shared.home.is_some(),
        "read-only descendants preserve the original scalar home"
    );
    assert!(
        shared
            .subslice(0, 1, StructuralAccess::MutableBorrow)
            .is_none()
    );
}

fn bounded_byte_collection() -> (TerminalExecution, Operation) {
    use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::structural_inputs::byte_arrays::storage::ArrayStorage;
    use crate::values::StructuralRuntimePlace;
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration};
    let (mut execution, mut operation) = runtime_subslice();
    let source = PlaceId::new(1).unwrap();
    let view_type = StructuralTypeId::new(2).unwrap();
    let bounded_type = StructuralTypeId::new(3).unwrap();
    let array_type = StructuralTypeId::new(4).unwrap();
    let byte_type = StructuralTypeId::new(5).unwrap();
    operation.kind = OperationKind::EstablishMutableElementView {
        region: terminal_psi::ElementViewRegion::WholeCollection,
        destination: PlaceId::new(2).unwrap(),
        source: terminal_psi::StructuralArgument {
            place: source,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        },
        element: byte_type,
    };
    let program = Arc::get_mut(&mut execution.program).unwrap();
    for (id, shape) in [
        (
            view_type,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
        ),
        (
            bounded_type,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::MutableBorrow),
                capacity: 3,
            }),
        ),
        (
            array_type,
            StructuralTypeShape::FixedArray {
                element: byte_type,
                length: 3,
            },
        ),
        (
            byte_type,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        ),
    ] {
        program.structural_types.insert(
            id,
            StructuralTypeDeclaration {
                id,
                identity: format!("type-{}", id.get()).into(),
                shape,
            },
        );
    }
    let machine = program
        .machines
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.structural_parameters[0].structural_type = bounded_type;
    machine
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations[0] = operation.clone();
    let source_value = execution.structural_values.get_mut(&source).unwrap();
    source_value.structural_type = bounded_type;
    let backing = StructuralRuntimePlace::from(&*source_value);
    execution
        .structural_byte_arrays
        .insert(backing.clone(), ByteSequenceView::new(vec![17, 128, 255]));
    execution.byte_sequence_values.insert(
        source,
        ByteSequenceBinding::MutableArray {
            array: ArrayStorage::from(backing),
            array_type,
            offset: 0,
            length: 3,
            referent: source_value.clone(),
        },
    );
    execution.element_view_values.clear();
    (execution, operation)
}

#[test]
fn bounded_byte_collection_establishment_preserves_home_and_capacity() {
    use crate::byte_sequences::binding::ByteSequenceBinding;
    let (mut execution, operation) = bounded_byte_collection();
    let source = PlaceId::new(1).unwrap();
    let destination = PlaceId::new(2).unwrap();
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    assert_eq!(execution.byte_sequence_length(source).unwrap(), 3);
    assert_eq!(execution.byte_sequence_length(destination).unwrap(), 3);
    assert_eq!(
        execution.structural_values[&source].structural_type,
        StructuralTypeId::new(3).unwrap()
    );
    assert_eq!(
        execution.structural_values[&destination].structural_type,
        StructuralTypeId::new(2).unwrap()
    );
    let ByteSequenceBinding::MutableArray {
        array,
        offset,
        length,
        ..
    } = &execution.byte_sequence_values[&destination]
    else {
        panic!("original array loan")
    };
    assert_eq!(array.referent.opaque_identity, 100);
    assert!(array.referent.path.is_empty());
    assert_eq!((*offset, *length), (0, 3));
    execution.values.insert(
        ValueId::new(2).unwrap(),
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(0),
        },
    );
    execution.values.insert(
        ValueId::new(3).unwrap(),
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(3),
        },
    );
    execution
        .execute_byte_sequence_write(&Operation {
            id: semantic_vocabulary::OperationId::new(2).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::ByteSequenceWrite {
                destination,
                index: ValueId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
                length: ValueId::new(3).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            },
        })
        .unwrap();
    assert_eq!(
        execution.structural_byte_array(100, &[]).unwrap(),
        &[17, 0, 255]
    );
    assert_eq!(execution.byte_sequence_read(source, 1).unwrap(), 0);
}

#[test]
fn bounded_byte_collection_host_input_publishes_storage_and_descriptor_atomically() {
    let (mut execution, operation) = bounded_byte_collection();
    execution.structural_byte_arrays.clear();
    execution.byte_sequence_values.clear();
    let input = crate::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue {
        argument_index: 0,
        path: Vec::new(),
        bytes: vec![17, 128, 255],
    };
    assert!(
        execution
            .bind_byte_arrays(&[input.clone(), input.clone()])
            .is_err()
    );
    assert!(execution.structural_byte_arrays.is_empty());
    assert!(execution.byte_sequence_values.is_empty());
    execution
        .bind_byte_arrays(std::slice::from_ref(&input))
        .unwrap();
    assert_eq!(
        execution
            .byte_sequence_length(PlaceId::new(1).unwrap())
            .unwrap(),
        3
    );
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    assert_eq!(
        execution
            .byte_sequence_read(PlaceId::new(2).unwrap(), 2)
            .unwrap(),
        255
    );
    assert!(execution.bind_byte_arrays(&[input]).is_err());
    assert_eq!(
        execution.structural_byte_array(100, &[]).unwrap(),
        &[17, 128, 255]
    );
}

#[test]
fn bounded_byte_collection_rejects_capacity_access_and_home_drift() {
    use crate::byte_sequences::binding::ByteSequenceBinding;
    for mutation in ["capacity", "access", "home", "element", "typed descriptor"] {
        let (mut execution, operation) = bounded_byte_collection();
        match mutation {
            "capacity" => {
                let declaration = Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .structural_types
                    .get_mut(&StructuralTypeId::new(3).unwrap())
                    .unwrap();
                let StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
                ) = &mut declaration.shape
                else {
                    panic!("bounded")
                };
                *capacity = 4;
            }
            "access" => {
                Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            "home" => {
                let ByteSequenceBinding::MutableArray { array, .. } = execution
                    .byte_sequence_values
                    .get_mut(&PlaceId::new(1).unwrap())
                    .unwrap()
                else {
                    panic!("array")
                };
                array.referent.opaque_identity = 101;
            }
            "element" => {
                Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .structural_types
                    .get_mut(&StructuralTypeId::new(5).unwrap())
                    .unwrap()
                    .shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                ));
            }
            "typed descriptor" => {
                Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .structural_types
                    .get_mut(&StructuralTypeId::new(2).unwrap())
                    .unwrap()
                    .shape = StructuralTypeShape::ElementView {
                    element: StructuralTypeId::new(5).unwrap(),
                };
            }
            _ => unreachable!(),
        }
        assert!(
            execution
                .execute_establish_element_view(&operation)
                .is_err(),
            "{mutation}"
        );
        assert!(
            !execution
                .structural_values
                .contains_key(&PlaceId::new(2).unwrap()),
            "{mutation}"
        );
        assert!(
            !execution
                .byte_sequence_values
                .contains_key(&PlaceId::new(2).unwrap()),
            "{mutation}"
        );
        assert!(execution.live_affine_frontier.is_empty(), "{mutation}");
        assert_eq!(
            execution.structural_byte_array(100, &[]).unwrap(),
            &[17, 128, 255],
            "{mutation}"
        );
    }
}

fn representation_byte_collection() -> (
    TerminalExecution,
    Operation,
    crate::values::StructuralRuntimePlace,
) {
    use crate::byte_sequences::ByteSequenceView;
    use terminal_psi::StructuralTypeDeclaration;
    let (mut execution, mut operation) = runtime_subslice();
    let source = PlaceId::new(1).unwrap();
    let byte_type = StructuralTypeId::new(1).unwrap();
    let view_type = StructuralTypeId::new(2).unwrap();
    let array_type = StructuralTypeId::new(3).unwrap();
    let element_type = StructuralTypeId::new(4).unwrap();
    operation.kind = OperationKind::EstablishMutableElementView {
        destination: PlaceId::new(2).unwrap(),
        source: terminal_psi::StructuralArgument {
            place: source,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        },
        element: element_type,
        region: terminal_psi::ElementViewRegion::RepresentationBytes {
            start: ValueId::new(1).unwrap(),
            end: ValueId::new(2).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        },
    };
    let program = Arc::get_mut(&mut execution.program).unwrap();
    for (id, shape) in [
        (
            byte_type,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        ),
        (
            view_type,
            StructuralTypeShape::ElementView {
                element: element_type,
            },
        ),
        (
            array_type,
            StructuralTypeShape::FixedArray {
                element: byte_type,
                length: 8,
            },
        ),
        (
            element_type,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            )),
        ),
    ] {
        program.structural_types.insert(
            id,
            StructuralTypeDeclaration {
                id,
                identity: format!("region-type-{}", id.get()).into(),
                shape,
            },
        );
    }
    let machine = program
        .machines
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.structural_parameters[0].structural_type = array_type;
    machine
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations[0] = operation.clone();
    let root = execution.structural_values.get_mut(&source).unwrap();
    root.structural_type = array_type;
    let backing = crate::values::StructuralRuntimePlace::from(&*root);
    execution.structural_byte_arrays.insert(
        backing.clone(),
        ByteSequenceView::new(vec![0, 1, 2, 3, 4, 5, 6, 7]),
    );
    execution.element_view_values.clear();
    for (number, value) in [(1, 2), (2, 6), (3, 2), (4, 0), (5, 1)] {
        execution.values.insert(
            ValueId::new(number).unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(value),
            },
        );
    }
    (execution, operation, backing)
}

fn representation_write(destination: u64, index: u64, value: u64) -> Operation {
    Operation {
        id: semantic_vocabulary::OperationId::new(20).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::ElementViewWrite {
            destination: PlaceId::new(destination).unwrap(),
            index: ValueId::new(index).unwrap(),
            value: ValueId::new(value).unwrap(),
            length: ValueId::new(3).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(20).unwrap(),
        },
    }
}

#[test]
fn representation_element_view_writes_live_bytes_through_transferred_descriptor() {
    let (mut execution, operation, backing) = representation_byte_collection();
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let destination = PlaceId::new(2).unwrap();
    let view = execution.element_view_values[&destination].clone();
    assert_eq!(view.len(), 2);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    assert_eq!(
        execution
            .read_representation_element(&view, 0, &[], scalar)
            .unwrap(),
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            value: IntegerValue::Unsigned(0x0302)
        }
    );
    // Ordinary argument binding changes only the descriptor place and access.
    let mut parameter =
        execution.program.machines[&execution.current_machine].structural_parameters[0].clone();
    parameter.place = PlaceId::new(3).unwrap();
    parameter.structural_type = StructuralTypeId::new(2).unwrap();
    let argument = terminal_psi::StructuralArgument {
        place: destination,
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    };
    let bound = execution
        .bind_element_view_arguments(
            &[parameter],
            &[argument],
            &[execution.structural_values[&destination].clone()],
        )
        .unwrap();
    execution.element_view_values.extend(bound);
    for (number, value) in [(6, 0x1234), (7, 0xabcd)] {
        execution.values.insert(
            ValueId::new(number).unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                value: IntegerValue::Unsigned(value),
            },
        );
    }
    execution
        .execute_element_view_write(&representation_write(3, 4, 6))
        .unwrap();
    execution
        .execute_element_view_write(&representation_write(3, 5, 7))
        .unwrap();
    let bytes = &execution.structural_byte_arrays[&backing];
    assert_eq!(
        (0..8)
            .map(|index| *bytes.get(index).unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 0x34, 0x12, 0xcd, 0xab, 6, 7]
    );
    // Original and nested descriptors observe current storage, never a snapshot.
    let nested = view.subslice(1, 2, StructuralAccess::SharedBorrow).unwrap();
    assert_eq!(
        execution
            .read_representation_element(&nested, 0, &[], scalar)
            .unwrap(),
        execution.values[&ValueId::new(7).unwrap()]
    );
    assert!(
        execution
            .write_representation_element(&nested, 0, execution.values[&ValueId::new(6).unwrap()])
            .is_err()
    );
    assert!(
        nested
            .subslice(0, 1, StructuralAccess::MutableBorrow)
            .is_none()
    );
}

#[test]
fn representation_element_view_refuses_bad_bounds_tiling_layout_and_authority() {
    for (start, end) in [(6, 2), (2, 9), (2, 5)] {
        let (mut execution, operation, _) = representation_byte_collection();
        for (number, value) in [(1, start), (2, end)] {
            execution.values.insert(
                ValueId::new(number).unwrap(),
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(value),
                },
            );
        }
        assert!(
            execution
                .execute_establish_element_view(&operation)
                .is_err()
        );
        assert!(
            !execution
                .element_view_values
                .contains_key(&PlaceId::new(2).unwrap())
        );
    }
    let (mut execution, operation, _) = representation_byte_collection();
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .structural_types
        .get_mut(&StructuralTypeId::new(4).unwrap())
        .unwrap()
        .shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    assert!(
        execution
            .execute_establish_element_view(&operation)
            .is_err()
    );
    let (mut execution, operation, _) = representation_byte_collection();
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let view = execution.element_view_values[&PlaceId::new(2).unwrap()].clone();
    let scalar = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    assert!(
        execution
            .write_representation_element(&view, 2, scalar)
            .is_err()
    );
    assert!(
        execution
            .read_representation_element(&view, 0, &[], ScalarType::Boolean)
            .is_err()
    );
    let shared = view.subslice(0, 2, StructuralAccess::SharedBorrow).unwrap();
    assert!(
        execution
            .write_representation_element(&shared, 0, scalar)
            .is_err()
    );
}

#[test]
fn representation_element_view_reads_exact_normalized_record_leaf() {
    let (mut execution, operation, _) = representation_byte_collection();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .structural_types
        .get_mut(&StructuralTypeId::new(4).unwrap())
        .unwrap()
        .shape = StructuralTypeShape::Record {
        fields: vec![terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            identity: "word".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
        }],
    };
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let read = Operation {
        id: semantic_vocabulary::OperationId::new(21).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            id: ValueId::new(9).unwrap(),
            scalar_type: scalar,
            qualifications: Default::default(),
        }),
        kind: OperationKind::ElementViewRead {
            source: PlaceId::new(2).unwrap(),
            index: ValueId::new(5).unwrap(),
            length: ValueId::new(3).unwrap(),
            path: vec![StructuralPathSegment::Field("word".into())],
            obligation: semantic_vocabulary::ObligationId::new(21).unwrap(),
        },
    };
    execution.execute_element_view_read(&read).unwrap();
    assert_eq!(
        execution.values[&ValueId::new(9).unwrap()],
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            value: IntegerValue::Unsigned(0x0504)
        }
    );
    let mut wrong = read;
    let OperationKind::ElementViewRead { path, .. } = &mut wrong.kind else {
        unreachable!()
    };
    path[0] = StructuralPathSegment::Field("other".into());
    assert!(execution.execute_element_view_read(&wrong).is_err());
}
