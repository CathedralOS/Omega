use super::*;
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
