use super::*;
use crate::{StructuralRuntimePlace, StructuralScalarRuntimeField};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, OperationResult, StructuralFieldDeclaration,
    StructuralFieldType, StructuralPlaceDeclaration, TerminalAffineCleanupAction,
};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn owned_successor() -> SuccessorEdge {
    let mut edge = successor();
    for argument in &mut edge.structural_arguments {
        argument.access = StructuralAccess::Owned;
    }
    edge
}

fn jump(edge: SuccessorEdge) -> Terminator {
    Terminator::Jump {
        edge: edge.edge,
        target: edge.target,
        arguments: edge.arguments,
        structural_arguments: edge.structural_arguments,
        trivial_affine_discards: edge.trivial_affine_discards,
        residual_affine_discards: Vec::new(),
    }
}

fn owned_execution(
    terminator: Terminator,
    multiplicity: StructuralMultiplicity,
) -> TerminalExecution {
    let mut execution = execution(terminator);
    let structural_type = StructuralTypeId::new(1).unwrap();
    execution
        .structural_types
        .get_mut(&structural_type)
        .unwrap()
        .shape = StructuralTypeShape::Record {
        fields: [
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            ScalarType::Boolean,
        ]
        .into_iter()
        .enumerate()
        .map(|(position, scalar_type)| StructuralFieldDeclaration {
            id: StructuralFieldId::new(position as u64 + 1).unwrap(),
            identity: format!("field{position}"),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(scalar_type),
        })
        .collect(),
    };
    execution.byte_sequence_values.clear();
    let target = execution.blocks.get_mut(&BlockId::new(2).unwrap()).unwrap();
    let mut sources = target.structural_parameters.clone();
    for source in &mut sources {
        source.access = StructuralAccess::Owned;
        source.multiplicity = multiplicity;
    }
    target.structural_parameters = sources.clone();
    for (position, parameter) in target.structural_parameters.iter_mut().enumerate() {
        parameter.place = PlaceId::new(position as u64 + 3).unwrap();
    }
    let integer_read = ValueDeclaration {
        id: ValueId::new(3).unwrap(),
        scalar_type: unsigned(0).scalar_type(),
    };
    target.operations = vec![
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(integer_read),
            kind: OperationKind::IntegerStructuralField {
                source: PlaceId::new(3).unwrap(),
                field: StructuralFieldId::new(1).unwrap(),
            },
        },
        Operation {
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(4).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: PlaceId::new(4).unwrap(),
                field: StructuralFieldId::new(2).unwrap(),
            },
        },
    ];
    target.terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: integer_read.id,
        cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
            [4, 3]
                .map(|place| TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(place).unwrap()))
                .to_vec()
        } else {
            Vec::new()
        },
    };
    let machine = execution
        .machines
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.parameters = target.parameters.clone();
    machine.structural_parameters = sources;
    machine.structural_places =
        machine
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
                        block: target.id,
                        position: parameter.position,
                    },
                }
            }))
            .collect();
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(9).unwrap(),
        scalar_type: integer_read.scalar_type,
    });
    machine.blocks = execution.blocks.clone();
    execution.live_affine_frontier =
        bind_affine_frontier(&machine.structural_parameters, &execution.structural_values).unwrap();
    for (place, integer, boolean) in [(1, 7, true), (2, 42, false)] {
        let root = &execution.structural_values[&PlaceId::new(place).unwrap()];
        for (field, value) in [
            (1, unsigned(integer)),
            (2, TerminalScalarValue::Boolean(boolean)),
        ] {
            execution.structural_scalar_fields.insert(
                StructuralScalarRuntimeField {
                    parent: StructuralRuntimePlace::from(root),
                    field: StructuralFieldId::new(field).unwrap(),
                },
                value,
            );
        }
    }
    execution
}

fn assert_owned_transition(terminator: Terminator, multiplicity: StructuralMultiplicity) {
    let mut execution = owned_execution(terminator, multiplicity);
    let original = execution.structural_values.clone();
    let fields = execution.structural_scalar_fields.clone();
    let frontier = execution.live_affine_frontier.clone();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values, original);
    assert_eq!(execution.live_affine_frontier, frontier);
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.structural_values.len(),
        if multiplicity == StructuralMultiplicity::Affine {
            2
        } else {
            4
        }
    );
    for (destination, source) in [(3, 2), (4, 1)] {
        assert_eq!(
            execution.structural_values[&PlaceId::new(destination).unwrap()],
            original[&PlaceId::new(source).unwrap()]
        );
        assert_eq!(
            execution
                .structural_values
                .contains_key(&PlaceId::new(source).unwrap()),
            multiplicity == StructuralMultiplicity::Unrestricted
        );
    }
    assert_eq!(
        execution.structural_scalar_fields, fields,
        "descriptor transfer preserves referent-keyed field contents"
    );
    assert_eq!(
        execution.values[&ValueId::new(1).unwrap()],
        TerminalScalarValue::Boolean(false)
    );
    assert_eq!(
        execution.values[&ValueId::new(2).unwrap()],
        TerminalScalarValue::Boolean(true)
    );
    assert_eq!(
        execution.live_affine_frontier,
        bind_affine_frontier(
            &execution.blocks[&BlockId::new(2).unwrap()].structural_parameters,
            &execution.structural_values
        )
        .unwrap()
    );
    for (value, expected) in [(3, unsigned(42)), (4, TerminalScalarValue::Boolean(true))] {
        meter.replenish(1).unwrap();
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(execution.values[&ValueId::new(value).unwrap()], expected);
    }
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(unsigned(42)))
    );
    assert!(execution.live_affine_frontier.is_empty());
    assert!(execution.live_claims.is_empty());
    assert_eq!(
        meter.usage().total_units(),
        4,
        "one edge, two field reads, one return; no replay"
    );
}

#[test]
fn owned_jump_moves_descriptors_and_reads_the_same_integer_and_boolean_backing_after_fuel() {
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Unrestricted,
    ] {
        assert_owned_transition(jump(owned_successor()), multiplicity);
    }
}

#[test]
fn owned_conditional_only_transfers_the_selected_arm() {
    for condition in [1, 2] {
        let selected = owned_successor();
        let invalid = SuccessorEdge {
            target: BlockId::new(999).unwrap(),
            ..selected.clone()
        };
        let (when_true, when_false) = if condition == 1 {
            (selected, invalid)
        } else {
            (invalid, selected)
        };
        assert_owned_transition(
            Terminator::Conditional {
                condition: ValueId::new(condition).unwrap(),
                when_true,
                when_false,
            },
            StructuralMultiplicity::Affine,
        );
    }
}

#[test]
fn owned_backedge_swaps_live_destination_roots_simultaneously() {
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Unrestricted,
    ] {
        let mut execution = owned_execution(jump(owned_successor()), multiplicity);
        let target = BlockId::new(2).unwrap();
        let first = owned_successor();
        let bindings = execution
            .prepare_block_bindings(target, &first.arguments, &first.structural_arguments)
            .unwrap();
        bindings.validate_discards(&execution, &[], &[]).unwrap();
        bindings.commit(&mut execution);
        let previous = execution.structural_values.clone();
        let fields = execution.structural_scalar_fields.clone();
        let arguments = [4, 3].map(|place| StructuralArgument {
            place: PlaceId::new(place).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        });
        let bindings = execution
            .prepare_block_bindings(target, &first.arguments, &arguments)
            .unwrap();
        bindings.validate_discards(&execution, &[], &[]).unwrap();
        bindings.commit(&mut execution);
        for (destination, source) in [(3, 4), (4, 3)] {
            assert_eq!(
                execution.structural_values[&PlaceId::new(destination).unwrap()],
                previous[&PlaceId::new(source).unwrap()]
            );
        }
        assert_eq!(
            execution.structural_values.len(),
            if multiplicity == StructuralMultiplicity::Affine {
                2
            } else {
                4
            }
        );
        assert_eq!(execution.structural_scalar_fields, fields);
        assert_eq!(
            execution.live_affine_frontier.len(),
            if multiplicity == StructuralMultiplicity::Affine {
                2
            } else {
                0
            }
        );
    }
}

#[test]
fn unrestricted_successors_may_duplicate_and_reuse_the_original_descriptor() {
    let mut execution = owned_execution(
        jump(owned_successor()),
        StructuralMultiplicity::Unrestricted,
    );
    let original = execution.structural_values.clone();
    let mut edge = owned_successor();
    edge.structural_arguments[1] = edge.structural_arguments[0].clone();
    let bindings = execution
        .prepare_block_bindings(edge.target, &edge.arguments, &edge.structural_arguments)
        .unwrap();
    bindings.validate_discards(&execution, &[], &[]).unwrap();
    bindings.commit(&mut execution);
    for place in [3, 4] {
        assert_eq!(
            execution.structural_values[&PlaceId::new(place).unwrap()],
            original[&PlaceId::new(2).unwrap()]
        );
    }
    for (place, value) in &original {
        assert_eq!(&execution.structural_values[place], value);
    }
    let original_edge = owned_successor();
    let bindings = execution
        .prepare_block_bindings(
            original_edge.target,
            &original_edge.arguments,
            &original_edge.structural_arguments,
        )
        .unwrap();
    bindings.validate_discards(&execution, &[], &[]).unwrap();
    bindings.commit(&mut execution);
    assert_eq!(
        execution.structural_values[&PlaceId::new(4).unwrap()],
        original[&PlaceId::new(1).unwrap()]
    );
    assert!(execution.live_affine_frontier.is_empty());
}

#[test]
fn an_explicit_discard_precedes_rebinding_the_same_destination_place() {
    let mut edge = owned_successor();
    let destination = PlaceId::new(3).unwrap();
    edge.trivial_affine_discards.push(destination);
    let mut execution = owned_execution(jump(edge), StructuralMultiplicity::Affine);
    let incoming = execution.structural_values[&PlaceId::new(2).unwrap()].clone();
    let mut previous = incoming.clone();
    previous.opaque_identity = 1000;
    execution.structural_values.insert(destination, previous);
    execution
        .live_affine_frontier
        .insert(StructuralAffineDiscard {
            place: destination,
            path: Vec::new(),
            structural_type: incoming.structural_type,
        });
    let before = execution.structural_values.clone();
    let frontier = execution.live_affine_frontier.clone();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values, before);
    assert_eq!(execution.live_affine_frontier, frontier);
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values[&destination], incoming);
    assert_eq!(execution.structural_values.len(), 2);
    assert_eq!(execution.live_affine_frontier.len(), 2);
}

#[test]
fn invalid_discard_suffix_cannot_partially_commit_a_valid_prefix() {
    for invalid in [3, 99] {
        let mut edge = owned_successor();
        let destination = PlaceId::new(3).unwrap();
        edge.trivial_affine_discards = vec![destination, PlaceId::new(invalid).unwrap()];
        let mut execution = owned_execution(jump(edge), StructuralMultiplicity::Affine);
        let mut value = execution.structural_values[&PlaceId::new(2).unwrap()].clone();
        value.opaque_identity = 1000;
        execution
            .live_affine_frontier
            .insert(StructuralAffineDiscard {
                place: destination,
                path: Vec::new(),
                structural_type: value.structural_type,
            });
        execution.structural_values.insert(destination, value);
        let before = execution.structural_values.clone();
        let frontier = execution.live_affine_frontier.clone();
        assert!(
            execution
                .resume(&mut TerminalFuelMeter::with_allowance(1))
                .is_err()
        );
        assert_eq!(execution.structural_values, before);
        assert_eq!(execution.live_affine_frontier, frontier);
    }
}

#[test]
fn malformed_owned_handoffs_leave_all_custody_uncommitted() {
    for mutation in [
        "duplicate",
        "alias",
        "missing",
        "partial",
        "source mode",
        "target owner",
        "qualification",
        "projected",
        "claim",
        "transfer and discard",
    ] {
        let mut edge = owned_successor();
        let mut execution = owned_execution(jump(edge.clone()), StructuralMultiplicity::Affine);
        let source = edge.structural_arguments[0].place;
        match mutation {
            "duplicate" => edge.structural_arguments[1].place = source,
            "alias" => {
                let identity = execution.structural_values[&source].opaque_identity;
                execution
                    .structural_values
                    .get_mut(&edge.structural_arguments[1].place)
                    .unwrap()
                    .opaque_identity = identity;
            }
            "missing" => {
                remove_affine_root(&mut execution.live_affine_frontier, source);
            }
            "partial" => {
                remove_affine_root(&mut execution.live_affine_frontier, source);
                execution
                    .live_affine_frontier
                    .insert(StructuralAffineDiscard {
                        place: source,
                        path: vec!["part".into()],
                        structural_type: StructuralTypeId::new(1).unwrap(),
                    });
            }
            "source mode" => {
                execution
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[1]
                    .access = StructuralAccess::SharedBorrow
            }
            "target owner" => {
                execution
                    .machines
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_places[2]
                    .kind = StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(1).unwrap(),
                    position: 0,
                }
            }
            "qualification" => execution
                .structural_values
                .get_mut(&source)
                .unwrap()
                .qualifications
                .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap()),
            "projected" => edge.structural_arguments[0].path.push("part".into()),
            "claim" => {
                execution.live_claims.insert(
                    semantic_vocabulary::ClaimId::new(1).unwrap(),
                    crate::LiveClaim {
                        place: Some(source),
                        path: Vec::new(),
                        multiplicity: Some(StructuralMultiplicity::Linear),
                    },
                );
            }
            "transfer and discard" => edge.trivial_affine_discards.push(source),
            _ => unreachable!(),
        }
        let original = execution.structural_values.clone();
        let frontier = execution.live_affine_frontier.clone();
        let claims = execution.live_claims.clone();
        let fields = execution.structural_scalar_fields.clone();
        let result = execution
            .prepare_block_bindings(edge.target, &edge.arguments, &edge.structural_arguments)
            .and_then(|bindings| {
                bindings.validate_discards(&execution, &edge.trivial_affine_discards, &[])
            });
        assert!(result.is_err(), "accepted {mutation}");
        assert_eq!(execution.structural_values, original);
        assert_eq!(execution.live_affine_frontier, frontier);
        assert_eq!(execution.live_claims, claims);
        assert_eq!(execution.structural_scalar_fields, fields);
    }
}
