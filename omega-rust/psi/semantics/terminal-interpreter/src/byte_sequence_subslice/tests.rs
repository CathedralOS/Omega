use std::collections::BTreeMap;

use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, StructuralDomainId,
    StructuralTypeId, ValueId,
};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_psi::{
    Block, OperationResult, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralTypeDeclaration,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

use super::*;
use crate::byte_sequence_view::ByteSequenceView;
use crate::{ExecutableMachine, LiveClaim, TerminalExecutionStatus, TerminalPayloadlessCaseValue};

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
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
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
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
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
        structural_primitive_entry_places: BTreeMap::new(),
        structural_scalar_fields: BTreeMap::new(),
        payloadless_case_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::from([(
            place(1),
            ByteSequenceView::new(vec![0, 128, 255, 7]),
        )]),
        live_affine_frontier: Default::default(),
        live_claims: BTreeMap::new(),
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
fn repeated_producer_shortens_to_empty_tail_without_changing_other_aliases() {
    let (mut execution, operation) = execution();
    let original = execution.byte_sequence_values[&place(1)].clone();
    execution
        .execute_byte_sequence_subslice(&operation)
        .unwrap();
    let first_tail = execution.byte_sequence_values[&place(2)].clone();
    execution
        .structural_values
        .insert(place(3), execution.structural_values[&place(2)].clone());
    execution
        .byte_sequence_values
        .insert(place(3), first_tail.clone());
    for length in (1..=3).rev() {
        let previous = execution.byte_sequence_values[&place(2)].clone();
        execution.byte_sequence_values.insert(place(1), previous);
        execution.values.insert(value(3), count(length));
        execution
            .execute_byte_sequence_subslice(&operation)
            .unwrap();
        let tail = &execution.byte_sequence_values[&place(2)];
        let offset = 5 - usize::try_from(length).unwrap();
        assert_eq!(tail.bytes(), &original.bytes()[offset..]);
        assert_eq!(tail.bytes().as_ptr(), original.bytes()[offset..].as_ptr());
        assert_eq!(
            execution.byte_sequence_values[&place(3)].bytes(),
            &[128, 255, 7]
        );
    }
    execution
        .byte_sequence_values
        .insert(place(1), execution.byte_sequence_values[&place(2)].clone());
    execution.values.insert(value(1), count(0));
    execution.values.insert(value(3), count(0));
    execution
        .execute_byte_sequence_subslice(&operation)
        .unwrap();
    assert_eq!(execution.byte_sequence_values[&place(2)].bytes(), &[]);
    assert_eq!(original.bytes(), &[0, 128, 255, 7]);
    assert_eq!(first_tail.bytes(), &[128, 255, 7]);
    assert!(execution.live_claims.is_empty());
    assert!(execution.live_affine_frontier.is_empty());
}

#[test]
fn source_descriptor_is_snapshotted_before_replacing_aliased_destination() {
    let (mut execution, mut operation) = execution();
    execution
        .execute_byte_sequence_subslice(&operation)
        .unwrap();
    let previous = execution.byte_sequence_values[&place(2)].clone();
    let OperationKind::ByteSequenceSubslice { source, .. } = &mut operation.kind else {
        panic!("subslice");
    };
    *source = place(2);
    execution.values.insert(value(3), count(3));
    execution
        .execute_byte_sequence_subslice(&operation)
        .unwrap();
    let tail = &execution.byte_sequence_values[&place(2)];
    assert_eq!(tail.bytes(), &[255, 7]);
    assert_eq!(tail.bytes().as_ptr(), previous.bytes()[1..].as_ptr());
    assert_eq!(previous.bytes(), &[128, 255, 7]);
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
                execution.payloadless_case_values.insert(
                    place(2),
                    TerminalPayloadlessCaseValue {
                        structural_type: result.structural_type,
                        result_case: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
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
                execution
                    .structural_types
                    .get_mut(&result.structural_type)
                    .unwrap()
                    .shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 4,
                });
            }
            _ => panic!("unknown mutation"),
        }
        let previous_values = execution.structural_values.clone();
        let previous_bytes = execution.byte_sequence_values.clone();
        let previous_scalars = execution.values.clone();
        let previous_claims = execution.live_claims.clone();
        let previous_frontier = execution.live_affine_frontier.clone();
        let previous_cases = execution.payloadless_case_values.clone();
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
        assert_eq!(
            execution.payloadless_case_values, previous_cases,
            "{mutation}"
        );
        assert_eq!(
            execution.byte_sequence_values.len(),
            previous_bytes.len(),
            "{mutation}"
        );
        for (place, previous) in previous_bytes {
            let current = &execution.byte_sequence_values[&place];
            assert_eq!(current.bytes(), previous.bytes(), "{mutation}");
            assert_eq!(
                current.bytes().as_ptr(),
                previous.bytes().as_ptr(),
                "{mutation}"
            );
        }
    }
}

#[test]
fn repeated_dispatch_rebinds_only_after_fuel_charge_and_resumes_once() {
    let (mut execution, _) = execution();
    let (mut uninterrupted, _) = self::execution();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for consumed in 0..=11 {
        let previous = execution.byte_sequence_values.clone();
        let previous_operation = execution.next_operation;
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(execution.next_operation, previous_operation);
        for (place, previous) in previous {
            assert_eq!(
                execution.byte_sequence_values[&place].bytes().as_ptr(),
                previous.bytes().as_ptr()
            );
            assert_eq!(
                execution.byte_sequence_values[&place].bytes(),
                previous.bytes()
            );
        }
        if consumed == 11 {
            break;
        }
        meter.replenish(1).unwrap();
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
    }
    let mut uninterrupted_meter = TerminalFuelMeter::with_allowance(11);
    assert!(matches!(
        uninterrupted.resume(&mut uninterrupted_meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.next_operation, 2);
    assert_eq!(execution.byte_sequence_values[&place(2)].bytes(), &[]);
    assert_eq!(execution.structural_values, uninterrupted.structural_values);
    assert_eq!(execution.values, uninterrupted.values);
    assert_eq!(meter.usage(), uninterrupted_meter.usage());
    assert_eq!(meter.usage().total_units(), 11);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(OperationId::new(2).unwrap()))
            .unwrap()
            .units(),
        4
    );
    for (place, bytes) in &execution.byte_sequence_values {
        assert_eq!(
            bytes.bytes(),
            uninterrupted.byte_sequence_values[place].bytes()
        );
    }
}
