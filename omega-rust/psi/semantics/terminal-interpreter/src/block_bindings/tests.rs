use super::*;
use crate::{ExecutableMachine, TerminalExecutionResult, TerminalExecutionStatus};
use semantic_vocabulary::{EdgeId, MachineId, ScalarType, StructuralTypeId};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, SuccessorEdge, TerminalMachineResult, Terminator, ValueDeclaration,
};

#[path = "tests/owned.rs"]
mod owned;

fn view_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: PlaceId::new(place).unwrap(),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn successor() -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(1).unwrap(),
        target: BlockId::new(2).unwrap(),
        arguments: vec![ValueId::new(2).unwrap(), ValueId::new(1).unwrap()],
        structural_arguments: vec![view_argument(2), view_argument(1)],
        trivial_affine_discards: Vec::new(),
    }
}

// Deliberately construct private runtime state to exercise defensive checks
// independently of verifier admission. Reusing destination places witnesses
// simultaneous replacement without admitting a cyclic artifact.
fn execution(terminator: Terminator) -> TerminalExecution {
    let machine_id = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let target = BlockId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let parameters = (1..=2)
        .map(|ordinal| ValueDeclaration {
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: ScalarType::Boolean,
        })
        .collect();
    let structural_parameters = (0..2)
        .map(|position| StructuralParameterDeclaration {
            place: PlaceId::new(u64::from(position) + 1).unwrap(),
            position,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        })
        .collect();
    let blocks = BTreeMap::from([
        (
            entry,
            Block {
                id: entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator,
            },
        ),
        (
            target,
            Block {
                id: target,
                parameters,
                structural_parameters,
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(2).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ),
    ]);
    let machine = ExecutableMachine {
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Unit,
        entry,
        blocks: blocks.clone(),
    };
    let structural_values = (1..=2)
        .map(|ordinal| {
            (
                PlaceId::new(ordinal).unwrap(),
                TerminalStructuralValue {
                    opaque_identity: ordinal + 100,
                    structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                },
            )
        })
        .collect();
    TerminalExecution {
        structural_types: BTreeMap::from([(
            structural_type,
            StructuralTypeDeclaration {
                id: structural_type,
                identity: "bytes".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
            },
        )]),
        machines: BTreeMap::from([(machine_id, machine)]),
        dynamic_scalar_calls: BTreeMap::new(),
        dynamic_descriptor_templates: BTreeMap::new(),
        dynamic_selection_templates: BTreeMap::new(),
        dynamic_descriptor_arguments: BTreeMap::new(),
        dynamic_parameters: BTreeMap::new(),
        boundary_machines: BTreeMap::new(),
        provider_candidates: Default::default(),
        provider_installation: BTreeMap::new(),
        blocks,
        values: BTreeMap::from([
            (ValueId::new(1).unwrap(), TerminalScalarValue::Boolean(true)),
            (
                ValueId::new(2).unwrap(),
                TerminalScalarValue::Boolean(false),
            ),
        ]),
        structural_values,
        structural_primitive_storage: BTreeMap::new(),
        structural_primitive_entry_places: BTreeMap::new(),
        primitive_local_identities:
            crate::primitive_storage::PrimitiveLocalIdentities::with_reserved_identities([
                1, 2, 101, 102,
            ]),
        structural_scalar_fields: BTreeMap::new(),
        structural_byte_sequence_fields: BTreeMap::new(),
        structural_byte_arrays: BTreeMap::new(),
        payloadless_case_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::from([
            (
                PlaceId::new(1).unwrap(),
                crate::ByteSequenceBinding::Immutable(crate::ByteSequenceView::new(vec![0, 128])),
            ),
            (
                PlaceId::new(2).unwrap(),
                crate::ByteSequenceBinding::Immutable(crate::ByteSequenceView::new(vec![
                    255, 7, 42,
                ])),
            ),
        ]),
        live_affine_frontier: Default::default(),
        live_claims: BTreeMap::new(),
        current_machine: machine_id,
        current: entry,
        next_operation: 0,
        call_stack: Vec::new(),
        result: None,
        crash: None,
        effects: Vec::new(),
    }
}

fn assert_swap_after_fuel(terminator: Terminator) {
    let mut execution = execution(terminator);
    let original_structural = execution.structural_values.clone();
    let original_scalars = execution.values.clone();
    let original_views = execution.byte_sequence_values.clone();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values, original_structural);
    assert_eq!(execution.values, original_scalars);
    assert_eq!(
        execution.byte_sequence_values[&PlaceId::new(1).unwrap()]
            .immutable()
            .unwrap()
            .bytes(),
        &[0, 128]
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    for (destination, source) in [(1, 2), (2, 1)] {
        assert_eq!(
            execution.structural_values[&PlaceId::new(destination).unwrap()],
            original_structural[&PlaceId::new(source).unwrap()]
        );
        assert_eq!(
            execution.values[&ValueId::new(destination).unwrap()],
            original_scalars[&ValueId::new(source).unwrap()]
        );
        assert_eq!(
            execution.byte_sequence_values[&PlaceId::new(destination).unwrap()]
                .immutable()
                .unwrap()
                .bytes()
                .as_ptr(),
            original_views[&PlaceId::new(source).unwrap()]
                .immutable()
                .unwrap()
                .bytes()
                .as_ptr()
        );
    }
    assert_eq!(
        execution.byte_sequence_values[&PlaceId::new(1).unwrap()]
            .immutable()
            .unwrap()
            .bytes(),
        &[255, 7, 42]
    );
    assert_eq!(
        execution.byte_sequence_values[&PlaceId::new(2).unwrap()]
            .immutable()
            .unwrap()
            .bytes(),
        &[0, 128]
    );
    assert!(execution.live_affine_frontier.is_empty());
    assert!(execution.live_claims.is_empty());
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn jump_swaps_views_and_scalars_once_after_fuel_replenishment() {
    let successor = successor();
    assert_swap_after_fuel(Terminator::Jump {
        edge: successor.edge,
        target: successor.target,
        arguments: successor.arguments,
        structural_arguments: successor.structural_arguments,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    });
}

#[test]
fn conditional_only_snapshots_selected_arm_and_resumes_once() {
    for (condition, when_true, when_false) in [
        (
            1,
            successor(),
            SuccessorEdge {
                target: BlockId::new(999).unwrap(),
                arguments: vec![ValueId::new(999).unwrap()],
                structural_arguments: vec![view_argument(999)],
                ..successor()
            },
        ),
        (
            2,
            SuccessorEdge {
                target: BlockId::new(999).unwrap(),
                arguments: vec![ValueId::new(999).unwrap()],
                structural_arguments: vec![view_argument(999)],
                ..successor()
            },
            successor(),
        ),
    ] {
        assert_swap_after_fuel(Terminator::Conditional {
            condition: ValueId::new(condition).unwrap(),
            when_true,
            when_false,
        });
    }
}

#[test]
fn malformed_view_bindings_reject_without_replacing_values() {
    for mutation in 0..7 {
        let successor = successor();
        let mut execution = execution(Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            trivial_affine_discards: Vec::new(),
        });
        let mut arguments = successor.structural_arguments;
        match mutation {
            0 => {
                arguments.pop();
            }
            1 => {
                arguments[1].place = PlaceId::new(999).unwrap();
            }
            2 => {
                arguments[1].access = StructuralAccess::Owned;
            }
            3 => {
                arguments[1].path.push("field".into());
            }
            4 => {
                execution.byte_sequence_values.remove(&arguments[1].place);
            }
            5 => {
                execution
                    .structural_values
                    .get_mut(&arguments[1].place)
                    .unwrap()
                    .structural_type = StructuralTypeId::new(999).unwrap();
            }
            6 => {
                execution
                    .blocks
                    .get_mut(&successor.target)
                    .unwrap()
                    .structural_parameters[1]
                    .is_self = true;
            }
            _ => unreachable!(),
        }
        let original_structural = execution.structural_values.clone();
        let original_scalars = execution.values.clone();
        assert!(
            execution
                .prepare_block_bindings(successor.target, &successor.arguments, &arguments)
                .is_err()
        );
        assert_eq!(execution.structural_values, original_structural);
        assert_eq!(execution.values, original_scalars);
    }
}

#[test]
fn mutable_field_loan_cannot_be_rebound_as_a_shared_or_owned_block_value() {
    use crate::{
        ByteSequenceBinding, ByteSequenceView, StructuralByteSequenceRuntimeField,
        StructuralRuntimePlace,
    };
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralPlaceDeclaration,
    };

    let edge = successor();
    let mut execution = execution(Terminator::ReturnUnit {
        edge: edge.edge,
        trivial_affine_discards: Vec::new(),
    });
    execution
        .prepare_block_bindings(edge.target, &edge.arguments, &edge.structural_arguments)
        .expect("the existing immutable-view fixture admits this successor");
    let parent_type = StructuralTypeId::new(2).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    execution.structural_types.insert(
        parent_type,
        StructuralTypeDeclaration {
            id: parent_type,
            identity: "Buffer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "bytes".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                }],
            },
        },
    );
    let parent_place = PlaceId::new(5).unwrap();
    let parent = TerminalStructuralValue {
        opaque_identity: 105,
        structural_type: parent_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let backing = StructuralByteSequenceRuntimeField {
        parent: StructuralRuntimePlace::from(&parent),
        field,
    };
    execution.structural_values.insert(parent_place, parent);
    execution
        .structural_byte_sequence_fields
        .insert(backing.clone(), ByteSequenceView::new(vec![10, 20]));

    let loan_place = PlaceId::new(1).unwrap();
    let mut formal = execution.blocks[&edge.target].structural_parameters[0].clone();
    formal.access = StructuralAccess::MutableBorrow;
    let mut prepared = execution
        .prepare_boundary_arguments(
            std::slice::from_ref(&formal),
            &[StructuralArgument {
                place: parent_place,
                path: vec![StructuralPathSegment::Field("bytes".into())],
                access: StructuralAccess::MutableBorrow,
            }],
        )
        .unwrap()
        .into_call_arguments(std::slice::from_ref(&formal))
        .unwrap();
    let referent = prepared.values.remove(0);
    let loan = prepared.byte_sequences.remove(&loan_place).unwrap();
    assert!(matches!(loan, ByteSequenceBinding::MutableField { .. }));
    loan.validate_mutable_referent(&execution.structural_types, &referent)
        .unwrap();
    execution
        .structural_values
        .insert(loan_place, referent.clone());
    execution.byte_sequence_values.insert(loan_place, loan);

    // Retain the real mutable source declaration and distinct block targets,
    // so Owned rejects a loan, not a missing source/declaration or arity error.
    let target = execution.blocks.get_mut(&edge.target).unwrap();
    for (position, parameter) in target.structural_parameters.iter_mut().enumerate() {
        parameter.place = PlaceId::new(position as u64 + 3).unwrap();
    }
    let machine = execution
        .machines
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.structural_parameters = vec![formal];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: loan_place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    machine
        .structural_places
        .extend(
            target
                .structural_parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: edge.target,
                        position: parameter.position,
                    },
                }),
        );
    let scalar_values = execution.values.clone();
    let frontier = execution.live_affine_frontier.clone();
    let backing_pointer = execution.structural_byte_sequence_fields[&backing]
        .bytes()
        .as_ptr();
    for access in [StructuralAccess::SharedBorrow, StructuralAccess::Owned] {
        execution
            .blocks
            .get_mut(&edge.target)
            .unwrap()
            .structural_parameters[1]
            .access = access;
        let mut arguments = edge.structural_arguments.clone();
        arguments[1].access = access;
        for erase_path in [false, true] {
            let mut supplied = referent.clone();
            if erase_path {
                supplied.path.clear();
            }
            execution.structural_values.insert(loan_place, supplied);
            let structural_values = execution.structural_values.clone();
            // Clearing a forged descriptor's path must not turn the binding
            // into an immutable snapshot or bypass its access-kind checks.
            assert!(
                matches!(
                    execution.prepare_block_bindings(edge.target, &edge.arguments, &arguments),
                    Err(TerminalInterpretError::VerifiedOperationMalformed)
                ),
                "access={access:?}, erase_path={erase_path}"
            );
            assert_eq!(execution.structural_values, structural_values);
            assert_eq!(execution.values, scalar_values);
            assert_eq!(execution.live_affine_frontier, frontier);
            let binding = &execution.byte_sequence_values[&loan_place];
            assert!(binding.immutable().is_err());
            binding
                .validate_mutable_referent(&execution.structural_types, &referent)
                .unwrap();
            assert_eq!(
                execution.structural_byte_sequence_fields[&backing].bytes(),
                &[10, 20]
            );
            assert_eq!(
                execution.structural_byte_sequence_fields[&backing]
                    .bytes()
                    .as_ptr(),
                backing_pointer
            );
        }
    }
}
