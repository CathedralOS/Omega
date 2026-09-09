use super::*;
use semantic_vocabulary::EdgeId;
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

mod array_tests;
#[cfg(test)]
mod write_tests;

fn place(ordinal: u64) -> PlaceId {
    PlaceId::new(ordinal).unwrap()
}
fn structural_type(ordinal: u64) -> StructuralTypeId {
    StructuralTypeId::new(ordinal).unwrap()
}
fn parameter(ordinal: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(ordinal),
        position: 0,
        is_self: false,
        structural_type: structural_type(3),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}
fn argument(ordinal: u64) -> StructuralArgument {
    StructuralArgument {
        place: place(ordinal),
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    }
}
fn field(element: u64) -> StructuralByteSequenceRuntimeField {
    StructuralByteSequenceRuntimeField {
        parent: StructuralRuntimePlace {
            opaque_identity: 101,
            path: vec![StructuralPathSegment::FixedIndex(element)],
        },
        field: StructuralFieldId::new(1).unwrap(),
    }
}

// Private runtime fixtures exercise defensive binding checks separately from
// source/verifier integration. The actual loan still originates at presentation.
fn execution() -> TerminalExecution {
    let machines = (1..=3)
        .map(|ordinal| {
            let entry = BlockId::new(ordinal).unwrap();
            (
                MachineId::new(ordinal).unwrap(),
                ExecutableMachine {
                    parameters: Vec::new(),
                    structural_parameters: if ordinal == 1 {
                        Vec::new()
                    } else {
                        vec![parameter(ordinal)]
                    },
                    structural_places: Vec::new(),
                    entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    result: TerminalMachineResult::Unit,
                    entry,
                    blocks: BTreeMap::from([(
                        entry,
                        Block {
                            id: entry,
                            parameters: Vec::new(),
                            structural_parameters: Vec::new(),
                            operations: Vec::new(),
                            terminator: Terminator::ReturnUnit {
                                edge: EdgeId::new(ordinal).unwrap(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    )]),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let current_machine = MachineId::new(1).unwrap();
    TerminalExecution {
        structural_types: BTreeMap::from([
            (
                structural_type(1),
                StructuralTypeDeclaration {
                    id: structural_type(1),
                    identity: "record".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "bytes".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                            ),
                        }],
                    },
                },
            ),
            (
                structural_type(2),
                StructuralTypeDeclaration {
                    id: structural_type(2),
                    identity: "array".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: structural_type(1),
                        length: 2,
                    },
                },
            ),
            (
                structural_type(3),
                StructuralTypeDeclaration {
                    id: structural_type(3),
                    identity: "view".into(),
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
                },
            ),
        ]),
        blocks: machines[&current_machine].blocks.clone(),
        machines,
        dynamic_scalar_calls: BTreeMap::new(),
        dynamic_descriptor_templates: BTreeMap::new(),
        dynamic_selection_templates: BTreeMap::new(),
        dynamic_descriptor_arguments: BTreeMap::new(),
        dynamic_parameters: BTreeMap::new(),
        boundary_machines: BTreeMap::new(),
        provider_candidates: BTreeSet::new(),
        provider_installation: BTreeMap::new(),
        values: BTreeMap::new(),
        structural_values: BTreeMap::from([(
            place(1),
            TerminalStructuralValue {
                opaque_identity: 101,
                structural_type: structural_type(2),
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        )]),
        structural_primitive_storage: BTreeMap::new(),
        structural_primitive_entry_places: BTreeMap::new(),
        primitive_local_identities:
            primitive_storage::PrimitiveLocalIdentities::with_reserved_identities([101]),
        structural_scalar_fields: BTreeMap::new(),
        structural_byte_arrays: BTreeMap::new(),
        structural_byte_sequence_fields: BTreeMap::from([
            (field(0), ByteSequenceView::new(vec![99])),
            (field(1), ByteSequenceView::new(vec![10])),
        ]),
        scalar_case_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::new(),
        live_affine_frontier: BTreeSet::new(),
        live_claims: BTreeMap::new(),
        current_machine,
        current: BlockId::new(1).unwrap(),
        next_operation: 0,
        call_stack: Vec::new(),
        result: None,
        crash: None,
        effects: Vec::new(),
    }
}

fn enter_provider(execution: &mut TerminalExecution) {
    let mut actual = argument(1);
    actual.path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("bytes".into()),
    ];
    let ordinary = execution
        .prepare_structural_call_arguments(
            MachineId::new(2).unwrap(),
            std::slice::from_ref(&actual),
        )
        .unwrap();
    let prepared = execution
        .prepare_boundary_arguments(&[parameter(11)], std::slice::from_ref(&actual))
        .unwrap();
    assert!(prepared.bytes.is_empty());
    assert!(prepared.buffers.is_empty());
    let prepared = prepared.into_call_arguments(&[parameter(2)]).unwrap();
    assert_eq!(ordinary.values, prepared.values);
    ordinary.byte_sequences[&place(2)]
        .validate_mutable_referent(&execution.structural_types, &prepared.values[0])
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
}

#[test]
fn mutable_field_forwarding_reads_current_backing_and_survives_nested_return_and_fuel() {
    let mut execution = execution();
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
    let original_path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("bytes".into()),
    ];
    for (expected, replacement) in [
        (&[10][..], &[255, 0][..]),
        (&[255, 0][..], &[][..]),
        (&[][..], &[7][..]),
    ] {
        let mut boundary = execution
            .resolve_boundary_arguments(&[parameter(11)], &[argument(3)])
            .unwrap();
        assert_eq!(boundary.values[0].path, original_path);
        assert_eq!(boundary.bytes[0].as_deref(), Some(expected));
        assert_eq!(boundary.buffers[0].capacity(), 8);
        assert_eq!(boundary.buffers[0].bytes(), expected);
        boundary.buffers[0].replace(replacement).unwrap();
        boundary.validate_writeback().unwrap();
        boundary.commit(&mut execution);
    }
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..3 {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(
            execution.structural_byte_sequence_field(
                101,
                &[StructuralPathSegment::FixedIndex(1)],
                field(1).field
            ),
            Some(&[7][..])
        );
        meter.replenish(1).unwrap();
    }
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
        Some(&[7][..])
    );
    assert_eq!(
        execution.structural_byte_sequence_field(
            101,
            &[StructuralPathSegment::FixedIndex(0)],
            field(0).field
        ),
        Some(&[99][..])
    );
}

#[test]
fn structural_result_entry_uses_prepared_field_loan_and_preserves_writeback_on_return() {
    let mut execution = execution();
    let callee_id = MachineId::new(2).unwrap();
    let token_type = structural_type(4);
    execution.structural_types.insert(
        token_type,
        StructuralTypeDeclaration {
            id: token_type,
            identity: "token".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    );
    let token = TerminalStructuralValue {
        opaque_identity: 202,
        structural_type: token_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    execution.structural_values.insert(place(4), token.clone());
    execution
        .live_affine_frontier
        .insert(StructuralAffineDiscard {
            place: place(4),
            path: Vec::new(),
            structural_type: token_type,
        });
    let token_parameter = StructuralParameterDeclaration {
        place: place(4),
        position: 1,
        is_self: false,
        structural_type: token_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let callee = execution.machines.get_mut(&callee_id).unwrap();
    callee.structural_parameters.push(token_parameter.clone());
    callee.result = TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
        place: place(6),
        structural_type: token_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    callee.blocks.get_mut(&callee.entry).unwrap().terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(2).unwrap(),
        source: place(4),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let callee_parameters = callee.structural_parameters.clone();
    execution
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(1).unwrap(),
        trivial_affine_discards: vec![place(5)],
    };
    let mut inline_argument = argument(1);
    inline_argument.path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("bytes".into()),
    ];
    let actuals = [
        inline_argument,
        StructuralArgument {
            place: place(4),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    ];
    // The ordinary resolver cannot represent this inline byte field as a view.
    assert!(
        resolve_structural_arguments(
            &execution.structural_types,
            &execution.structural_values,
            &actuals
        )
        .is_err()
    );
    let prepared = execution
        .prepare_boundary_arguments(&[parameter(11), token_parameter], &actuals)
        .unwrap()
        .into_call_arguments(&callee_parameters)
        .unwrap();
    execution
        .begin_structural_result_call(
            callee_id,
            StructuralOperationResult {
                place: place(5),
                structural_type: token_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            &[],
            &actuals,
            prepared,
            &[],
            Vec::new(),
        )
        .unwrap();
    assert_eq!(execution.current_machine, callee_id);
    assert_eq!(execution.structural_values[&place(2)].path, actuals[0].path);
    let mut boundary = execution
        .resolve_boundary_arguments(&[parameter(11)], &[argument(2)])
        .unwrap();
    boundary.buffers[0].replace(&[0, 255]).unwrap();
    boundary.validate_writeback().unwrap();
    boundary.commit(&mut execution);
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.current_machine, MachineId::new(1).unwrap());
    assert_eq!(execution.structural_values[&place(5)], token);
    assert!(!execution.structural_values.contains_key(&place(4)));
    assert!(execution.byte_sequence_values.is_empty());
    let rebound = execution
        .resolve_boundary_arguments(&[parameter(11)], &actuals[..1])
        .unwrap();
    assert_eq!(rebound.buffers[0].bytes(), &[0, 255]);
    assert_eq!(rebound.buffers[0].capacity(), 8);
    assert_eq!(rebound.values[0].path, actuals[0].path);
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn mutable_field_reborrow_rejects_absent_or_mismatched_binding_and_immutable_use() {
    for mutation in 0..8 {
        let mut execution = execution();
        enter_provider(&mut execution);
        let binding = execution.byte_sequence_values[&place(2)].clone();
        assert!(binding.immutable().is_err());
        match mutation {
            0 => {
                execution.byte_sequence_values.remove(&place(2));
            }
            1 => {
                execution
                    .structural_values
                    .get_mut(&place(2))
                    .unwrap()
                    .opaque_identity += 1;
            }
            2 => {
                execution
                    .structural_values
                    .get_mut(&place(2))
                    .unwrap()
                    .path
                    .clear();
            }
            3 => {
                execution.structural_byte_sequence_fields.remove(&field(1));
            }
            4 => {
                execution.byte_sequence_values.insert(
                    place(2),
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(vec![10])),
                );
            }
            mutation => {
                let ByteSequenceBinding::MutableField {
                    field,
                    capacity,
                    parent_type,
                    ..
                } = execution.byte_sequence_values.get_mut(&place(2)).unwrap()
                else {
                    panic!("mutable binding");
                };
                match mutation {
                    5 => field.parent.path = vec![StructuralPathSegment::FixedIndex(0)],
                    6 => *capacity = 9,
                    7 => *parent_type = structural_type(2),
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            execution
                .resolve_boundary_arguments(&[parameter(11)], &[argument(2)])
                .is_err(),
            "mutation {mutation}"
        );
    }
    let mut execution = execution();
    enter_provider(&mut execution);
    let mut shared = parameter(3);
    shared.access = StructuralAccess::SharedBorrow;
    let mut actual = argument(2);
    actual.access = StructuralAccess::SharedBorrow;
    assert!(
        execution
            .bind_byte_sequence_arguments(
                &[shared],
                &[actual],
                &[execution.structural_values[&place(2)].clone()]
            )
            .is_err()
    );
}
