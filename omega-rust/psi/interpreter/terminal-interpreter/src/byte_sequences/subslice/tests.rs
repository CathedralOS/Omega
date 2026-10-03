use crate::byte_sequences::subslice::{
    IntegerSign, IntegerType, IntegerValue, Operation, OperationKind, StructuralMultiplicity,
    StructuralPlaceKind, TerminalExecution, TerminalScalarValue, TerminalStructuralValue,
};
use std::collections::BTreeMap;
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

use crate::execution::ExecutableMachine;
use crate::execution::LiveClaim;
use crate::structural_identities::LocalStructuralIdentities;
use crate::values::TerminalScalarCaseValue;
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, StructuralDomainId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, OperationResult, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralTypeDeclaration,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

fn place(ordinal: u64) -> PlaceId {
    PlaceId::new(ordinal).unwrap()
}

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn count(count: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(count),
    }
}

// Private runtime construction exercises rebinding and defensive failures.
// It does not establish verifier admission or source-produced cyclic evidence.
fn execution() -> (TerminalExecution, Operation) {
    let machine_id = MachineId::new(1).unwrap();
    let block_id = BlockId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let operation = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: place(2),
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ByteSequenceSubslice {
            source: place(1),
            start: value(1),
            end: value(3),
            length: value(3),
            obligation: ObligationId::new(1).unwrap(),
        },
    };
    let blocks = BTreeMap::from([(
        block_id,
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: block_id,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place(1),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value(3),
                        scalar_type: count(0).scalar_type(),
                    }),
                    kind: OperationKind::ByteSequenceLength { source: place(1) },
                },
                operation.clone(),
            ],
            terminator: Terminator::Jump {
                edge: EdgeId::new(1).unwrap(),
                target: block_id,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place(2),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        },
    )]);
    let machine = ExecutableMachine {
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place(2),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation.id,
                structural_type,
            },
        }],
        entry_claims: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Unit,
        entry: block_id,
        blocks: blocks.clone(),
    };
    let execution = TerminalExecution {
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
            (value(1), count(1)),
            (value(2), count(5)),
            (value(3), count(4)),
        ]),
        structural_values: BTreeMap::from([(
            place(1),
            TerminalStructuralValue {
                opaque_identity: 101,
                structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        )]),
        structural_primitive_storage: BTreeMap::new(),
        reference_referents: BTreeMap::new(),
        structural_element_view_fields: BTreeMap::new(),
        structural_primitive_entry_places: BTreeMap::new(),
        local_structural_identities: LocalStructuralIdentities::with_reserved_identities([
            1, 2, 101,
        ]),
        structural_scalar_fields: BTreeMap::new(),
        structural_byte_sequence_fields: BTreeMap::new(),
        structural_byte_arrays: BTreeMap::new(),
        scalar_case_values: BTreeMap::new(),
        scalar_array_values: BTreeMap::new(),
        element_view_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::from([(
            place(1),
            crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                crate::byte_sequences::ByteSequenceView::new(vec![0, 128, 255, 7]),
            ),
        )]),
        live_affine_frontier: Default::default(),
        live_claims: BTreeMap::new(),
        placed_view_occurrences: BTreeMap::new(),
        current_machine: machine_id,
        current: block_id,
        next_operation: 0,
        call_stack: Vec::new(),
        result: None,
        crash: None,
        effects: Vec::new(),
    };
    (execution, operation)
}

#[test]
fn failed_replacement_preserves_descriptors_and_live_custody() {
    for mutation in [
        "missing destination bytes",
        "missing destination metadata",
        "wrong destination type",
        "destination path",
        "destination qualifications",
        "destination identity",
        "destination claim",
        "destination ownership",
        "conflicting case",
        "wrong producer",
        "source claim",
        "source ownership",
        "source path",
        "source qualifications",
        "missing source bytes",
        "missing source metadata",
        "wrong source type",
        "stale length",
        "reversed bounds",
        "end beyond length",
        "wrong count type",
        "narrow count type",
        "signed count",
        "count overflow",
        "missing count",
        "result multiplicity",
        "result qualifications",
        "result projected qualifications",
        "result claims",
        "wrong result type",
        "wrong carrier",
    ] {
        let (mut execution, mut operation) = execution();
        execution
            .execute_byte_sequence_subslice(&operation)
            .unwrap();
        let domain = StructuralDomainId::new(1).unwrap();
        let claim = ClaimId::new(1).unwrap();
        let wrong_type = StructuralTypeId::new(2).unwrap();
        let OperationResult::Structural(result) = &mut operation.result else {
            panic!("structural result");
        };
        match mutation {
            "missing destination bytes" => {
                execution.byte_sequence_values.remove(&place(2));
            }
            "missing destination metadata" => {
                execution.structural_values.remove(&place(2));
            }
            "wrong destination type" => {
                execution
                    .structural_values
                    .get_mut(&place(2))
                    .unwrap()
                    .structural_type = wrong_type
            }
            "destination path" => execution
                .structural_values
                .get_mut(&place(2))
                .unwrap()
                .path
                .push("field".into()),
            "destination qualifications" => execution
                .structural_values
                .get_mut(&place(2))
                .unwrap()
                .qualifications
                .push(domain),
            "destination identity" => {
                execution
                    .structural_values
                    .get_mut(&place(2))
                    .unwrap()
                    .opaque_identity = 99
            }
            "destination claim" | "source claim" => {
                execution.live_claims.insert(
                    claim,
                    LiveClaim {
                        place: Some(place(if mutation == "source claim" { 1 } else { 2 })),
                        path: vec!["field".into()],
                        multiplicity: Some(StructuralMultiplicity::Affine),
                    },
                );
            }
            "destination ownership" | "source ownership" => {
                execution
                    .live_affine_frontier
                    .insert(StructuralAffineDiscard {
                        place: place(if mutation == "source ownership" { 1 } else { 2 }),
                        path: vec!["field".into()],
                        structural_type: result.structural_type,
                    });
            }
            "conflicting case" => {
                execution.scalar_case_values.insert(
                    place(2),
                    TerminalScalarCaseValue {
                        structural_type: result.structural_type,
                        result_case: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                        fields: vec![],
                    },
                );
            }
            "wrong producer" => operation.id = OperationId::new(99).unwrap(),
            "source path" => execution
                .structural_values
                .get_mut(&place(1))
                .unwrap()
                .path
                .push("field".into()),
            "source qualifications" => execution
                .structural_values
                .get_mut(&place(1))
                .unwrap()
                .qualifications
                .push(domain),
            "missing source bytes" => {
                execution.byte_sequence_values.remove(&place(1));
            }
            "missing source metadata" => {
                execution.structural_values.remove(&place(1));
            }
            "wrong source type" => {
                execution
                    .structural_values
                    .get_mut(&place(1))
                    .unwrap()
                    .structural_type = wrong_type
            }
            "stale length" => {
                execution.values.insert(value(3), count(3));
            }
            "reversed bounds" => {
                execution.values.insert(value(1), count(5));
            }
            "end beyond length" => {
                let OperationKind::ByteSequenceSubslice { end, .. } = &mut operation.kind else {
                    panic!("subslice");
                };
                *end = value(2);
            }
            "wrong count type" => {
                execution
                    .values
                    .insert(value(1), TerminalScalarValue::Boolean(true));
            }
            "narrow count type" => {
                execution.values.insert(
                    value(1),
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        value: IntegerValue::Unsigned(1),
                    },
                );
            }
            "signed count" => {
                execution.values.insert(
                    value(1),
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                        value: IntegerValue::Signed(1),
                    },
                );
            }
            "count overflow" => {
                execution
                    .values
                    .insert(value(1), count(u128::from(u64::MAX) + 1));
            }
            "missing count" => {
                execution.values.remove(&value(1));
            }
            "result multiplicity" => result.multiplicity = StructuralMultiplicity::Affine,
            "result qualifications" => result.qualifications.push(domain),
            "result projected qualifications" => {
                result
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec!["field".into()],
                        domain,
                    })
            }
            "result claims" => result.claims.push(StructuralResultClaimBinding {
                claim,
                path: Vec::new(),
            }),
            "wrong result type" => result.structural_type = wrong_type,
            "wrong carrier" => {
                execution.mutate_structural_type(result.structural_type, |declaration| {
                    declaration.shape =
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            capacity: 4,
                        });
                });
            }
            _ => panic!("unknown mutation"),
        }
        let previous_values = execution.structural_values.clone();
        let previous_bytes = execution.byte_sequence_values.clone();
        let previous_scalars = execution.values.clone();
        let previous_claims = execution.live_claims.clone();
        let previous_frontier = execution.live_affine_frontier.clone();
        let previous_cases = execution.scalar_case_values.clone();
        assert!(
            execution
                .execute_byte_sequence_subslice(&operation)
                .is_err(),
            "{mutation}"
        );
        assert_eq!(execution.structural_values, previous_values, "{mutation}");
        assert_eq!(execution.values, previous_scalars, "{mutation}");
        assert_eq!(execution.live_claims, previous_claims, "{mutation}");
        assert_eq!(
            execution.live_affine_frontier, previous_frontier,
            "{mutation}"
        );
        assert_eq!(execution.scalar_case_values, previous_cases, "{mutation}");
        assert_eq!(
            execution.byte_sequence_values.len(),
            previous_bytes.len(),
            "{mutation}"
        );
        for (place, previous) in previous_bytes {
            let current = &execution.byte_sequence_values[&place];
            assert_eq!(
                current.immutable().unwrap().bytes(),
                previous.immutable().unwrap().bytes(),
                "{mutation}"
            );
            assert_eq!(
                current.immutable().unwrap().bytes().as_ptr(),
                previous.immutable().unwrap().bytes().as_ptr(),
                "{mutation}"
            );
        }
    }
}

fn mutable_array_execution() -> (TerminalExecution, Operation) {
    use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::structural_inputs::byte_arrays::storage::ArrayStorage;
    use crate::values::StructuralRuntimePlace;
    let (mut execution, mut operation) = execution();
    let OperationResult::Structural(result) = &mut operation.result else {
        panic!("descriptor result")
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let OperationKind::ByteSequenceSubslice { end, .. } = &mut operation.kind else {
        panic!("subslice")
    };
    *end = value(2);
    execution.values.insert(value(2), count(3));
    let program = std::sync::Arc::get_mut(&mut execution.program).unwrap();
    program
        .structural_types
        .get_mut(&StructuralTypeId::new(1).unwrap())
        .unwrap()
        .shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::MutableBorrow),
    });
    program.structural_types.insert(
        StructuralTypeId::new(2).unwrap(),
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "backing".into(),
            shape: StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(3).unwrap(),
                length: 4,
            },
        },
    );
    program.structural_types.insert(
        StructuralTypeId::new(3).unwrap(),
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(3).unwrap(),
            identity: "byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
    );
    let block = program
        .machines
        .get_mut(&MachineId::new(1).unwrap())
        .unwrap()
        .blocks
        .get_mut(&BlockId::new(1).unwrap())
        .unwrap();
    block.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    block.operations[1] = operation.clone();
    let referent = execution.structural_values[&place(1)].clone();
    let backing = StructuralRuntimePlace::from(&referent);
    execution
        .structural_byte_arrays
        .insert(backing.clone(), ByteSequenceView::new(vec![0, 128, 255, 7]));
    execution.byte_sequence_values.insert(
        place(1),
        ByteSequenceBinding::MutableArray {
            array: ArrayStorage::from(backing),
            array_type: StructuralTypeId::new(2).unwrap(),
            offset: 0,
            length: 4,
            referent,
        },
    );
    (execution, operation)
}

#[test]
fn mutable_byte_range_keeps_original_backing_and_independent_descriptor_custody() {
    use crate::byte_sequences::binding::ByteSequenceBinding;
    let (mut execution, operation) = mutable_array_execution();
    execution
        .execute_byte_sequence_subslice(&operation)
        .unwrap();
    assert_eq!(execution.byte_sequence_length(place(1)).unwrap(), 4);
    assert_eq!(execution.byte_sequence_length(place(2)).unwrap(), 2);
    let debt = StructuralAffineDiscard {
        place: place(2),
        path: Vec::new(),
        structural_type: StructuralTypeId::new(1).unwrap(),
    };
    assert_eq!(
        execution.live_affine_frontier,
        std::collections::BTreeSet::from([debt])
    );
    execution.values.insert(value(4), count(2));
    execution.values.insert(
        value(5),
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(42),
        },
    );
    let write = Operation {
        id: OperationId::new(9).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::ByteSequenceWrite {
            destination: place(2),
            index: value(1),
            value: value(5),
            length: value(4),
            obligation: ObligationId::new(2).unwrap(),
        },
    };
    execution.execute_byte_sequence_write(&write).unwrap();
    assert_eq!(execution.byte_sequence_read(place(2), 1).unwrap(), 42);
    assert_eq!(
        execution.structural_byte_array(101, &[]).unwrap(),
        &[0, 128, 42, 7]
    );
    let borrowed = &execution.byte_sequence_values[&place(2)];
    let referent = &execution.structural_values[&place(2)];
    let shared = execution
        .reborrow_byte_sequence_binding(borrowed, referent, StructuralAccess::SharedBorrow)
        .unwrap();
    assert_eq!(shared.immutable().unwrap().bytes(), &[128, 42]);
    assert!(
        execution
            .reborrow_byte_sequence_binding(&shared, referent, StructuralAccess::MutableBorrow)
            .is_err()
    );
    assert!(matches!(
        borrowed,
        ByteSequenceBinding::MutableArray {
            offset: 1,
            length: 2,
            ..
        }
    ));
}

#[test]
fn mutable_byte_range_rejects_changed_home_extent_and_authority_before_publication() {
    use crate::byte_sequences::binding::ByteSequenceBinding;
    for mutation in ["home", "extent", "shared", "linear", "end"] {
        let (mut execution, mut operation) = mutable_array_execution();
        match mutation {
            "home" | "extent" => {
                let ByteSequenceBinding::MutableArray { array, length, .. } =
                    execution.byte_sequence_values.get_mut(&place(1)).unwrap()
                else {
                    panic!("array")
                };
                if mutation == "home" {
                    array.referent.opaque_identity = 102;
                } else {
                    *length = 5;
                }
            }
            "shared" => {
                std::sync::Arc::get_mut(&mut execution.program)
                    .unwrap()
                    .machines
                    .get_mut(&MachineId::new(1).unwrap())
                    .unwrap()
                    .blocks
                    .get_mut(&BlockId::new(1).unwrap())
                    .unwrap()
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            "linear" => {
                let OperationResult::Structural(result) = &mut operation.result else {
                    panic!("descriptor")
                };
                result.multiplicity = StructuralMultiplicity::Linear;
            }
            "end" => {
                execution.values.insert(value(2), count(5));
            }
            _ => unreachable!(),
        }
        assert!(
            execution
                .execute_byte_sequence_subslice(&operation)
                .is_err(),
            "{mutation}"
        );
        assert!(
            !execution.structural_values.contains_key(&place(2)),
            "{mutation}"
        );
        assert!(
            !execution.byte_sequence_values.contains_key(&place(2)),
            "{mutation}"
        );
        assert!(execution.live_affine_frontier.is_empty(), "{mutation}");
        assert_eq!(
            execution.structural_byte_array(101, &[]).unwrap(),
            &[0, 128, 255, 7],
            "{mutation}"
        );
    }
}
