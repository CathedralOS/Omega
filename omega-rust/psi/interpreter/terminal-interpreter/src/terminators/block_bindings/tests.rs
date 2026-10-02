use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::execution::ExecutableMachine;
use crate::results::TerminalExecutionStatus;
use crate::structural_identities::LocalStructuralIdentities;
use crate::terminators::block_bindings::{
    BTreeMap, BlockId, PlaceId, StructuralArgument, StructuralPlaceKind, TerminalExecution,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue, ValueId,
};
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::StructuralRuntimePlace;
use semantic_vocabulary::{EdgeId, MachineId, ScalarType, StructuralTypeId};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, SuccessorEdge, TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

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
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
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
            qualifications: Default::default(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
        structural_cases: BTreeMap::new(),
        program: std::sync::Arc::new(crate::program::AdmittedTerminalProgram::for_test(
            BTreeMap::from([(machine_id, machine)]),
            BTreeMap::from([(
                structural_type,
                StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "bytes".into(),
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }),
                },
            )]),
        )),
        dynamic_parameters: BTreeMap::new(),
        provider_installation: BTreeMap::new(),
        assumed_bindings: Vec::new(),
        values: BTreeMap::from([
            (ValueId::new(1).unwrap(), TerminalScalarValue::Boolean(true)),
            (
                ValueId::new(2).unwrap(),
                TerminalScalarValue::Boolean(false),
            ),
        ]),
        structural_values,
        structural_primitive_storage: BTreeMap::new(),
        reference_referents: BTreeMap::new(),
        structural_primitive_entry_places: BTreeMap::new(),
        local_structural_identities: LocalStructuralIdentities::with_reserved_identities([
            1, 2, 101, 102,
        ]),
        structural_scalar_fields: BTreeMap::new(),
        structural_byte_sequence_fields: BTreeMap::new(),
        structural_byte_arrays: BTreeMap::new(),
        scalar_case_values: BTreeMap::new(),
        scalar_array_values: BTreeMap::new(),
        element_view_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::from([
            (
                PlaceId::new(1).unwrap(),
                crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                    crate::byte_sequences::ByteSequenceView::new(vec![0, 128]),
                ),
            ),
            (
                PlaceId::new(2).unwrap(),
                crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                    crate::byte_sequences::ByteSequenceView::new(vec![255, 7, 42]),
                ),
            ),
        ]),
        live_affine_frontier: Default::default(),
        live_claims: BTreeMap::new(),
        placed_view_occurrences: BTreeMap::new(),
        current_machine: machine_id,
        current: entry,
        next_operation: 0,
        call_stack: Vec::new(),
        result: None,
        crash: None,
        effects: Vec::new(),
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
                std::sync::Arc::get_mut(&mut execution.program)
                    .map(|program| &mut program.machines)
                    .unwrap()
                    .get_mut(&execution.current_machine)
                    .unwrap()
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
                .prepare_block_bindings(successor.target, &successor.arguments, &arguments, true)
                .is_err()
        );
        assert_eq!(execution.structural_values, original_structural);
        assert_eq!(execution.values, original_scalars);
    }
}

#[test]
fn mutable_field_loan_attenuation_preserves_referent_and_rejects_forged_custody() {
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
        .prepare_block_bindings(
            edge.target,
            &edge.arguments,
            &edge.structural_arguments,
            true,
        )
        .expect("the existing immutable-view fixture admits this successor");
    let parent_type = StructuralTypeId::new(2).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    execution.insert_structural_type(StructuralTypeDeclaration {
        id: parent_type,
        identity: "Buffer".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "bytes".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 8,
                }),
            }],
        },
    });
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
    let mut formal = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap()
        .blocks[&edge.target]
        .structural_parameters[0]
        .clone();
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
    let referent = prepared.values.remove(&loan_place).unwrap();
    let loan = prepared.byte_sequences.remove(&loan_place).unwrap();
    assert!(matches!(loan, ByteSequenceBinding::MutableField { .. }));
    loan.validate_mutable_referent(&execution.program.structural_types, &referent)
        .unwrap();
    execution
        .structural_values
        .insert(loan_place, referent.clone());
    execution.byte_sequence_values.insert(loan_place, loan);

    // Retain the real mutable source declaration and distinct block targets,
    // so Owned rejects a loan, not a missing source/declaration or arity error.
    let target = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap()
        .blocks
        .get_mut(&edge.target)
        .unwrap();
    for (position, parameter) in target.structural_parameters.iter_mut().enumerate() {
        parameter.place = PlaceId::new(position as u64 + 3).unwrap();
    }
    let target_parameters = target.structural_parameters.clone();
    let machine = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
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
            target_parameters
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
        std::sync::Arc::get_mut(&mut execution.program)
            .map(|program| &mut program.machines)
            .unwrap()
            .get_mut(&execution.current_machine)
            .unwrap()
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
            let outcome =
                execution.prepare_block_bindings(edge.target, &edge.arguments, &arguments, true);
            if access == StructuralAccess::SharedBorrow && !erase_path {
                // Ordinary access attenuation retains the exact referent and
                // produces the same immutable snapshot used by call binding.
                let prepared = outcome.expect("a valid mutable loan can lend shared access");
                let destination = PlaceId::new(4).unwrap();
                assert_eq!(prepared.structural[&destination], referent);
                let mut snapshot = prepared.byte_sequences[&destination]
                    .immutable()
                    .unwrap()
                    .clone();
                assert_eq!(snapshot.bytes(), &[10, 20]);
                assert!(prepared.affine_sources.is_empty());
                assert!(prepared.affine_destinations.is_empty());
                snapshot.replace_byte(0, 99).unwrap();
                assert_eq!(snapshot.bytes(), &[99, 20]);
            } else {
                // Removing the recorded field path cannot manufacture a
                // different loan, and lending never transfers owned custody.
                assert!(
                    matches!(
                        outcome,
                        Err(TerminalInterpretError::VerifiedOperationMalformed)
                    ),
                    "access={access:?}, erase_path={erase_path}"
                );
            }
            assert_eq!(execution.structural_values, structural_values);
            assert_eq!(execution.values, scalar_values);
            assert_eq!(execution.live_affine_frontier, frontier);
            let binding = &execution.byte_sequence_values[&loan_place];
            assert!(binding.immutable().is_err());
            binding
                .validate_mutable_referent(&execution.program.structural_types, &referent)
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

#[test]
fn mutable_borrowed_places_forward_with_exact_source_authority_and_debt() {
    use terminal_psi::{StructuralAffineDiscard, StructuralPlaceDeclaration};

    for shape in [
        StructuralTypeShape::Record { fields: Vec::new() },
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
    ] {
        for mutation in 0..6 {
            let mut edge = successor();
            for argument in &mut edge.structural_arguments {
                argument.access = StructuralAccess::MutableBorrow;
            }
            let mut execution = execution(Terminator::ReturnUnit {
                edge: edge.edge,
                trivial_affine_discards: Vec::new(),
            });
            let structural_type = StructuralTypeId::new(1).unwrap();
            execution.mutate_structural_type(structural_type, |declaration| {
                declaration.shape = shape.clone();
            });
            execution.byte_sequence_values.clear();
            let machine = std::sync::Arc::get_mut(&mut execution.program)
                .unwrap()
                .machines
                .get_mut(&execution.current_machine)
                .unwrap();
            let target = machine.blocks.get_mut(&edge.target).unwrap();
            for (position, parameter) in target.structural_parameters.iter_mut().enumerate() {
                parameter.access = StructuralAccess::MutableBorrow;
                parameter.place = PlaceId::new(position as u64 + 3).unwrap();
            }
            machine.structural_parameters = target
                .structural_parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    let mut source = parameter.clone();
                    source.place = PlaceId::new(position as u64 + 1).unwrap();
                    source
                })
                .collect();
            machine.structural_places = machine
                .structural_parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: false,
                    },
                })
                .chain(target.structural_parameters.iter().map(|parameter| {
                    StructuralPlaceDeclaration {
                        id: parameter.place,
                        kind: StructuralPlaceKind::BlockParameter {
                            block: edge.target,
                            position: parameter.position,
                        },
                    }
                }))
                .collect();
            match mutation {
                1 => machine.structural_parameters[0].access = StructuralAccess::SharedBorrow,
                2 => machine
                    .structural_places
                    .retain(|place| place.id != PlaceId::new(1).unwrap()),
                3 => {
                    machine.structural_parameters[0].structural_type =
                        StructuralTypeId::new(9).unwrap()
                }
                4 => {
                    execution
                        .live_affine_frontier
                        .insert(StructuralAffineDiscard {
                            place: PlaceId::new(1).unwrap(),
                            path: Vec::new(),
                            structural_type,
                        });
                }
                5 => edge.structural_arguments[0]
                    .path
                    .push(terminal_psi::StructuralPathSegment::Field("missing".into())),
                _ => {}
            }
            let prepared = execution.prepare_block_bindings(
                edge.target,
                &edge.arguments,
                &edge.structural_arguments,
                true,
            );
            if mutation != 0 {
                assert!(prepared.is_err(), "mutation {mutation}");
                continue;
            }
            let prepared =
                prepared.expect("mutable borrowed storage has no descriptor-family restriction");
            prepared.validate_discards(&execution, &[], &[]).unwrap();
            prepared.commit(&mut execution);
            assert_eq!(
                execution.structural_values[&PlaceId::new(3).unwrap()].opaque_identity,
                102
            );
            assert_eq!(
                execution.structural_values[&PlaceId::new(4).unwrap()].opaque_identity,
                101
            );
            assert!(execution.live_affine_frontier.is_empty());
        }
    }
}
