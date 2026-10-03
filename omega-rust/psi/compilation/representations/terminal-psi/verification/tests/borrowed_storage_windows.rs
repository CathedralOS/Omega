//! Borrowed-storage restoration windows: `MoveStructuralField` opens a
//! restoration debt beneath a mutable-borrowed machine parameter and only an
//! exact `StoreStructuralField` at the same canonical hole closes it. These
//! fixtures replay the checked transform's evidence through Terminal
//! verification: the debt, not the producer's claim, is what every non-crash
//! exit, edge, join, and operation is checked against.

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Block, CrashCause, MachineContract, Operation, OperationKind,
    OperationResult, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};
use terminal_verifier::{ModuleError, validate_module};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn cell() -> StructuralTypeId {
    id(1, StructuralTypeId::new)
}

fn envelope() -> StructuralTypeId {
    id(2, StructuralTypeId::new)
}

fn flag() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn left() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn right() -> StructuralFieldId {
    id(2, StructuralFieldId::new)
}

fn structural_types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: cell(),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: flag(),
                    identity: "flag".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: envelope(),
            identity: "Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: left(),
                        identity: "left".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                    StructuralFieldDeclaration {
                        id: right(),
                        identity: "right".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                ],
            },
        },
    ]
}

fn machine() -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(1, MachineId::new),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(1, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: id(1, PlaceId::new),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: id(2, PlaceId::new),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id(1, OperationId::new),
                    structural_type: cell(),
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(1, BlockId::new),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            operations: vec![extract(id(1, OperationId::new), 2), repair(2)],
            terminator: Terminator::ReturnUnit {
                edge: id(1, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id(1, ContractId::new),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// `MoveStructuralField` opening the `left` window on the borrowed envelope
/// parameter, producing the moved `Cell` at `result`.
fn extract(operation: OperationId, result: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(result, PlaceId::new),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::MoveStructuralField {
            source: id(1, PlaceId::new),
            path: Vec::new(),
            field: Some(left()),
        },
    }
}

/// `StoreStructuralField` reseating the `left` window with `value`.
fn repair(value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(2, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: id(1, PlaceId::new),
            path: Vec::new(),
            field: Some(left()),
            value: StructuralArgument {
                place: id(value, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    }
}

/// A fresh `Cell` established whole for `producer`, stored at `place`.
fn fresh_cell(operation: OperationId, place: u64, flag_value: ValueId) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(place, PlaceId::new),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishRecord {
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: flag(),
                value: RecordFieldValue::Scalar {
                    value: flag_value,
                    range_obligation: None,
                },
            }],
        },
    }
}

/// A `BooleanConstant` scalar producer.
fn boolean_constant(operation: OperationId, result: ValueId, value: bool) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value },
    }
}

/// A Boolean field read: canonical `path` walks to the record containing
/// `field` beneath `source`.
fn boolean_read(
    operation: OperationId,
    result: ValueId,
    source: u64,
    path: Vec<CanonicalStructuralPathSegment>,
    field: StructuralFieldId,
) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            source: id(source, PlaceId::new),
            path,
            field,
        },
    }
}

fn window_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(1, MachineId::new),
        structural_types: structural_types(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine()],
    }
}

fn assert_rejects(module: &TerminalModule, expected: ModuleError) {
    assert_eq!(
        validate_module(module).unwrap_err(),
        expected,
        "unexpected validation outcome"
    );
}

/// An ordinary call completes a copy record whose inline bytes retain Utf8;
/// the caller replaces an existing borrowed field with that exact value.
fn called_byte_record_window(scalar_arguments: bool) -> TerminalModule {
    use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet};
    let mut module = window_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("Cell is a record");
    };
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    fields[0].field_type = StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
        predicates,
    };

    let mut callee = machine();
    callee.id = id(2, MachineId::new);
    callee.entry = id(2, BlockId::new);
    callee.blocks[0].id = callee.entry;
    callee.contract.id = id(2, ContractId::new);
    callee.structural_parameters.clear();
    callee.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(10, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(10, OperationId::new),
                structural_type: cell(),
            },
        },
        StructuralPlaceDeclaration {
            id: id(11, PlaceId::new),
            kind: StructuralPlaceKind::Result,
        },
    ];
    callee.result = TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: id(11, PlaceId::new),
        structural_type: cell(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    let mut establishment = extract(id(10, OperationId::new), 10);
    establishment.kind = OperationKind::EstablishZeroedStructural {};
    callee.blocks[0].operations = vec![establishment];
    callee.blocks[0].terminator = Terminator::ReturnStructural {
        edge: id(10, EdgeId::new),
        source: id(10, PlaceId::new),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(callee);

    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3, OperationId::new),
            structural_type: cell(),
        },
    });
    let mut call = extract(id(3, OperationId::new), 3);
    call.kind = if scalar_arguments {
        OperationKind::CallStructuralWithScalarArguments {
            callee: id(2, MachineId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    } else {
        OperationKind::CallStructural {
            callee: id(2, MachineId::new),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        }
    };
    caller.blocks[0].operations = vec![call, extract(id(1, OperationId::new), 2), repair(3)];
    module
}

#[test]
fn ordinary_byte_record_result_repairs_borrowed_field() {
    for scalar_arguments in [false, true] {
        let module = called_byte_record_window(scalar_arguments);
        terminal_verifier::verify_module(&module, &Default::default(), &Default::default())
            .unwrap_or_else(|error| {
                panic!("completed byte record scalar_arguments={scalar_arguments}: {error:?}")
            });
    }
}

#[test]
fn ordinary_byte_record_repair_rejects_forged_completion_and_storage() {
    let valid = called_byte_record_window(false);
    for corruption in 0..8 {
        let mut module = valid.clone();
        match corruption {
            0 => module.machines[0].blocks[0].operations.rotate_left(1),
            1 => {
                let OperationKind::StoreStructuralField { value, .. } =
                    &mut module.machines[0].blocks[0].operations[2].kind
                else {
                    panic!("repair")
                };
                value.access = StructuralAccess::SharedBorrow;
            }
            2 => {
                let OperationKind::StoreStructuralField { field, .. } =
                    &mut module.machines[0].blocks[0].operations[2].kind
                else {
                    panic!("repair")
                };
                *field = Some(right());
            }
            3 => {
                module.machines[0].blocks[0].operations.pop();
            }
            4 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[0].result
                else {
                    panic!("call result")
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            5 => {
                let mut other = module.structural_types[0].clone();
                other.id = id(3, StructuralTypeId::new);
                other.identity = "OtherCell".into();
                module.structural_types.push(other);
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    panic!("callee result")
                };
                result.structural_type = id(3, StructuralTypeId::new);
            }
            6 => {
                module.machines[0].blocks[0].operations[0].kind =
                    OperationKind::EstablishRecord { fields: Vec::new() };
            }
            7 => {
                module.machines[0].structural_places[2].kind =
                    StructuralPlaceKind::OperationResult {
                        producer: id(99, OperationId::new),
                        structural_type: cell(),
                    };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "corruption {corruption} accepted"
        );
    }
}

fn local_byte_window_module() -> TerminalModule {
    use terminal_psi::control_flow::ByteSequenceOperand;
    use terminal_psi::{ByteSequenceCarrier, StructuralPathSegment};
    let mut module = window_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type =
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 4 });
    let bytes_type = id(3, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: bytes_type,
        identity: "Snapshot".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    });
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(3, OperationId::new),
        structural_type: envelope(),
    };
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(5, OperationId::new),
            structural_type: bytes_type,
        },
    });
    let operations = &mut machine.blocks[0].operations;
    // The displaced subtree is affine and consumed by its exact repair.
    // The containing local stays owned and becomes whole again afterward.
    let OperationResult::Structural(displaced) = &mut operations[0].result else {
        unreachable!()
    };
    displaced.multiplicity = StructuralMultiplicity::Affine;
    operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(3, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: id(1, PlaceId::new),
                structural_type: envelope(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishZeroedStructural {},
        },
    );
    let path = vec![StructuralPathSegment::Field("left".into())];
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(4, OperationId::new),
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: id(1, ValueId::new),
            scalar_type: semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
        }),
        kind: OperationKind::StructuralByteSequenceFieldLength {
            source: id(1, PlaceId::new),
            path: path.clone(),
            field: flag(),
        },
    });
    let operand = ByteSequenceOperand::Field {
        source: StructuralArgument {
            place: id(1, PlaceId::new),
            access: StructuralAccess::SharedBorrow,
            path,
        },
        field: flag(),
        length: id(1, ValueId::new),
    };
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(5, OperationId::new),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(3, PlaceId::new),
            structural_type: bytes_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishByteSequenceConcatenation {
            left: operand.clone(),
            right: operand,
            capacity_obligation: id(1, semantic_vocabulary::ObligationId::new),
        },
    });
    module
}

#[test]
fn repaired_local_window_expires_byte_field_length_observations() {
    let mut module = local_byte_window_module();
    validate_module(&module).expect("a fresh observation after exact local repair is valid");
    let mut open_hole = module.clone();
    open_hole.machines[0]
        .structural_places
        .retain(|place| place.id != id(3, PlaceId::new));
    let operations = &mut open_hole.machines[0].blocks[0].operations;
    // Remove the later concatenation so freshness cannot mask the custody
    // check: even an unrestricted local cannot observe absent storage.
    operations.pop();
    let observation = operations.remove(3);
    operations.insert(2, observation);
    assert_rejects(
        &open_hole,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(4, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
    let operations = &mut module.machines[0].blocks[0].operations;
    let observation = operations.remove(3);
    operations.insert(1, observation);
    assert_rejects(
        &module,
        ModuleError::InvalidByteSequenceConcatenation(id(5, OperationId::new)),
    );
}

#[test]
fn local_byte_field_length_expires_after_mutation_through_element_view() {
    use terminal_psi::{StructuralPathSegment, control_flow::ByteSequenceOperand};
    for sibling_view in [false, true] {
        let mut module = local_byte_window_module();
        let array_type = id(4, StructuralTypeId::new);
        let view_type = id(5, StructuralTypeId::new);
        module.structural_types.extend([
            StructuralTypeDeclaration {
                id: array_type,
                identity: "Cells".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: cell(),
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: view_type,
                identity: "CellView".into(),
                shape: StructuralTypeShape::ElementView { element: cell() },
            },
        ]);
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            unreachable!()
        };
        for field in fields {
            field.field_type = StructuralFieldType::Structural(array_type);
        }
        let caller = &mut module.machines[0];
        caller
            .structural_places
            .retain(|place| place.id != id(2, PlaceId::new));
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: id(4, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(6, OperationId::new),
                structural_type: view_type,
            },
        });
        let operations = &mut caller.blocks[0].operations;
        operations.retain(|operation| {
            !matches!(
                operation.kind,
                OperationKind::MoveStructuralField { .. }
                    | OperationKind::StoreStructuralField { .. }
            )
        });
        let OperationKind::StructuralByteSequenceFieldLength { path, .. } = &mut operations[1].kind
        else {
            unreachable!()
        };
        path.push(StructuralPathSegment::FixedIndex(0));
        let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
            &mut operations[2].kind
        else {
            unreachable!()
        };
        for operand in [left, right] {
            let ByteSequenceOperand::Field { source, .. } = operand else {
                unreachable!()
            };
            source.path.push(StructuralPathSegment::FixedIndex(0));
        }
        operations.insert(
            1,
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(6, OperationId::new),
                result: OperationResult::Structural(StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place: id(4, PlaceId::new),
                    structural_type: view_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::EstablishMutableElementView {
                    region: terminal_psi::ElementViewRegion::WholeCollection,
                    destination: id(4, PlaceId::new),
                    element: cell(),
                    source: StructuralArgument {
                        place: id(1, PlaceId::new),
                        access: StructuralAccess::MutableBorrow,
                        path: vec![StructuralPathSegment::Field(
                            if sibling_view { "right" } else { "left" }.into(),
                        )],
                    },
                },
            },
        );
        operations.insert(
            2,
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(7, OperationId::new),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: id(2, MachineId::new),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: id(4, PlaceId::new),
                        access: StructuralAccess::MutableBorrow,
                        path: vec![StructuralPathSegment::FixedIndex(0)],
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        );
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut caller.blocks[0].terminator
        else {
            unreachable!()
        };
        trivial_affine_discards.push(id(4, PlaceId::new));
        let mut callee = machine();
        callee.id = id(2, MachineId::new);
        callee.contract.id = id(2, ContractId::new);
        callee.structural_parameters[0].structural_type = cell();
        callee.structural_parameters[0].place = id(10, PlaceId::new);
        callee.structural_places.truncate(1);
        callee.structural_places[0].id = id(10, PlaceId::new);
        callee.entry = id(2, BlockId::new);
        callee.blocks[0].id = callee.entry;
        callee.blocks[0].terminator = Terminator::ReturnUnit {
            edge: id(2, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        };
        callee.blocks[0].operations.clear();
        module.machines.push(callee);
        validate_module(&module).expect(
            "post-call length observes the current local field through a valid mutable view loan",
        );
        let operations = &mut module.machines[0].blocks[0].operations;
        let observation = operations.remove(3);
        operations.insert(2, observation);
        if sibling_view {
            validate_module(&module)
                .expect("mutation through a view of the sibling array preserves the observation");
        } else {
            assert_rejects(
                &module,
                ModuleError::InvalidByteSequenceConcatenation(id(5, OperationId::new)),
            );
        }
    }
}

fn block_window_module() -> TerminalModule {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    let mut body = machine.blocks[0].clone();
    body.id = id(2, BlockId::new);
    let mut parameter = machine.structural_parameters[0].clone();
    parameter.place = id(5, PlaceId::new);
    body.structural_parameters.push(parameter);
    if let OperationKind::MoveStructuralField { source, .. } = &mut body.operations[0].kind {
        *source = id(5, PlaceId::new);
    }
    if let OperationKind::StoreStructuralField { destination, .. } = &mut body.operations[1].kind {
        *destination = id(5, PlaceId::new);
    }
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(5, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: body.id,
            position: 0,
        },
    });
    machine.blocks[0].operations.clear();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(2, EdgeId::new),
        target: body.id,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: id(1, PlaceId::new),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(body);
    module
}

#[test]
fn a_borrowed_block_parameter_can_open_and_restore_a_window() {
    validate_module(&block_window_module()).expect("block binding preserves borrowed storage");
}

#[test]
fn a_block_window_preserves_disjoint_access_through_its_borrowed_parameter() {
    let mut module = block_window_module();
    module.machines[0].blocks[1].operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            5,
            vec![CanonicalStructuralPathSegment::Field(right())],
            flag(),
        ),
    );
    validate_module(&module).expect("aliasing does not make disjoint fields absent");
}

#[test]
fn a_block_window_rejects_absent_storage_read_through_its_borrowed_parameter() {
    let mut module = block_window_module();
    module.machines[0].blocks[1].operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            5,
            vec![CanonicalStructuralPathSegment::Field(left())],
            flag(),
        ),
    );
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(5, OperationId::new),
            place: id(5, PlaceId::new),
        },
    );
}

#[test]
fn the_original_parameter_cannot_repair_after_transferring_its_borrow() {
    let mut module = block_window_module();
    module.machines[0].blocks[1].operations[1] = repair(2);
    assert_rejects(
        &module,
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(2, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_shared_block_parameter_cannot_open_a_window() {
    let mut module = block_window_module();
    module.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!();
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert_rejects(
        &module,
        ModuleError::InvalidBorrowedStorageWindow {
            operation: id(1, OperationId::new),
            place: id(5, PlaceId::new),
        },
    );
}

#[test]
fn a_restored_window_can_repeat_through_a_loop_carried_borrow() {
    let mut module = block_window_module();
    let mut backedge = module.machines[0].blocks[0].terminator.clone();
    let Terminator::Jump {
        edge,
        structural_arguments,
        ..
    } = &mut backedge
    else {
        unreachable!();
    };
    *edge = id(3, EdgeId::new);
    structural_arguments[0].place = id(5, PlaceId::new);
    module.machines[0].blocks[1].terminator = backedge;
    validate_module(&module).expect("restored loop-carried borrow has a concrete storage origin");
}

#[test]
fn a_disjoint_sibling_stays_observable() {
    let mut module = window_module();
    let operations = &mut module.machines[0].blocks[0].operations;
    // Between the move and the repair the `right` subtree was never absent:
    // the debt is path-granular, not whole-root.
    operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            1,
            vec![CanonicalStructuralPathSegment::Field(right())],
            flag(),
        ),
    );
    validate_module(&module).expect("disjoint sibling stays observable");
}

#[test]
fn a_different_same_typed_value_repairs_the_window() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3, OperationId::new),
            structural_type: cell(),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    // Repair with a freshly established Cell rather than the extracted one:
    // the debt names the hole and the required type, not the moved identity.
    operations.insert(
        0,
        boolean_constant(id(5, OperationId::new), id(1, ValueId::new), false),
    );
    operations.insert(
        2,
        fresh_cell(id(3, OperationId::new), 3, id(1, ValueId::new)),
    );
    let OperationKind::StoreStructuralField { value, .. } = &mut operations[3].kind else {
        unreachable!()
    };
    value.place = id(3, PlaceId::new);
    validate_module(&module).expect("replacement repair validates");
}

#[test]
fn missing_repair_rejects_at_the_exit() {
    let mut module = window_module();
    module.machines[0].blocks[0].operations.pop();
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRestorationPending {
            machine: id(1, MachineId::new),
            block: id(1, BlockId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn an_early_return_through_a_second_block_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.pop();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(2, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(3, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    });
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRestorationPending {
            machine: id(1, MachineId::new),
            block: id(2, BlockId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_store_at_the_wrong_place_rejects() {
    let mut module = window_module();
    // `right` was never vacated; the only open window is `left`.
    let OperationKind::StoreStructuralField { field, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *field = Some(right());
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRepairMismatch {
            operation: id(2, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_scalar_field_cannot_open_a_window() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(5, OperationId::new),
            structural_type: cell(),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    // `flag` is a scalar leaf of the Cell, not a structural subtree; a window
    // can only open on a declared `Structural` field.
    operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(5, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: id(3, PlaceId::new),
                structural_type: cell(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::MoveStructuralField {
                source: id(1, PlaceId::new),
                path: vec![terminal_psi::StructuralPathSegment::Field("left".into())],
                field: Some(flag()),
            },
        },
    );
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: id(5, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_repair_value_of_the_wrong_type_rejects() {
    let mut module = window_module();
    // A whole owned Envelope is not the Cell the open window requires.
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(6, PlaceId::new),
            position: 1,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(6, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::StoreStructuralField { value, .. } =
        &mut machine.blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    value.place = id(6, PlaceId::new);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: id(2, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn repeated_extraction_of_the_open_window_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(4, OperationId::new),
            structural_type: cell(),
        },
    });
    machine.blocks[0]
        .operations
        .insert(1, extract(id(4, OperationId::new), 4));
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(4, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_stale_read_under_the_open_window_rejects() {
    let mut module = window_module();
    module.machines[0].blocks[0].operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            1,
            vec![CanonicalStructuralPathSegment::Field(left())],
            flag(),
        ),
    );
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(5, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_scalar_field_beside_the_hole_stays_observable() {
    // A direct scalar read on the windowed root touches only its own field:
    // re-reading a counter beside an open `left` window reaches no absent
    // storage, as the element swap's repair-index re-read does.
    let mut module = window_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!("Envelope is a record")
    };
    let count = id(3, StructuralFieldId::new);
    fields.push(StructuralFieldDeclaration {
        id: count,
        identity: "count".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
    });
    module.machines[0].blocks[0].operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            1,
            Vec::new(),
            count,
        ),
    );
    validate_module(&module).expect("a sibling scalar field is not beneath the hole");
}

#[test]
fn the_window_cannot_cross_an_edge() {
    for (name, argument, parameter_type) in [
        (
            "the exact absent subtree",
            vec![terminal_psi::StructuralPathSegment::Field("left".into())],
            cell(),
        ),
        ("the whole owner", Vec::new(), envelope()),
    ] {
        let mut module = window_module();
        let machine = &mut module.machines[0];
        machine.blocks[0].operations.pop();
        machine.blocks[0].terminator = Terminator::Jump {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(1, PlaceId::new),
                path: argument,
                access: StructuralAccess::SharedBorrow,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: id(5, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: parameter_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            id: id(2, BlockId::new),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(3, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(5, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(2, BlockId::new),
                position: 0,
            },
        });
        let result = validate_module(&module);
        assert!(
            matches!(
                result,
                Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place,
                }) if edge == id(2, EdgeId::new) && place == id(1, PlaceId::new)
            ),
            "{name} across the edge rejected unexpectedly: {result:?}"
        );
    }
}

fn branch_local_window_module() -> TerminalModule {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    // The true arm opens the window; the false arm does not. The merge cannot
    // reconstruct one frontier, so the join itself rejects.
    machine.blocks[0].operations = vec![boolean_constant(
        id(5, OperationId::new),
        id(1, ValueId::new),
        true,
    )];
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(1, ValueId::new),
        when_true: SuccessorEdge {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(3, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let open_arm = Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        operations: vec![extract(id(6, OperationId::new), 2)],
        terminator: Terminator::Jump {
            edge: id(4, EdgeId::new),
            target: id(4, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    };
    let quiet_arm = Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: id(5, EdgeId::new),
            target: id(4, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    };
    let merge = Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(4, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(6, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    };
    // The extracted result place must name its producer on the moved arm.
    machine
        .structural_places
        .retain(|place| !matches!(place.kind, StructuralPlaceKind::OperationResult { .. }));
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(2, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(6, OperationId::new),
            structural_type: cell(),
        },
    });
    machine.blocks.append(&mut vec![open_arm, quiet_arm, merge]);
    module
}

#[test]
fn a_join_must_carry_the_window_through_both_arms() {
    let module = branch_local_window_module();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralFrontierJoinMismatch(id(4, BlockId::new)),
    );
}

#[test]
fn a_repaired_branch_local_window_has_no_unrestricted_disposal_debt() {
    let mut module = branch_local_window_module();
    module.machines[0].blocks[1].operations.push(repair(2));
    validate_module(&module).expect("repaired copyable storage can rejoin a quiet branch");
    let frontiers = terminal_verifier::reconstruct_structural_ownership_frontiers(&module)
        .expect("exact ownership replay");
    let joined = frontiers.machines()[0]
        .block_entry(id(4, BlockId::new))
        .expect("merge frontier");
    assert!(joined.owned_places().is_empty());
    assert!(joined.restoration_debt().is_empty());
}

#[test]
fn a_branch_local_displaced_value_does_not_dominate_the_join() {
    let mut module = branch_local_window_module();
    module.machines[0].blocks[1].operations.push(repair(2));
    module.machines[0].blocks[3].operations.push(boolean_read(
        id(7, OperationId::new),
        id(2, ValueId::new),
        2,
        Vec::new(),
        flag(),
    ));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(7, OperationId::new),
            place: id(2, PlaceId::new),
        }),
        "copyability is not availability",
    );
}

#[test]
fn a_fresh_replacement_leaves_only_affine_displaced_values_in_the_frontier() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let mut module = branch_local_window_module();
        let machine = &mut module.machines[0];
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(3, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(7, OperationId::new),
                structural_type: cell(),
            },
        });
        let operations = &mut machine.blocks[1].operations;
        let OperationResult::Structural(result) = &mut operations[0].result else {
            unreachable!()
        };
        result.multiplicity = multiplicity;
        operations.push(fresh_cell(id(7, OperationId::new), 3, id(1, ValueId::new)));
        operations.push(repair(3));
        // The extracted value remains readable after replacement on its own
        // branch. Only an affine value leaves a disposal obligation at the join.
        operations.push(boolean_read(
            id(8, OperationId::new),
            id(2, ValueId::new),
            2,
            Vec::new(),
            flag(),
        ));
        let expected = if multiplicity == StructuralMultiplicity::Unrestricted {
            Ok(())
        } else {
            Err(ModuleError::OwnedStructuralFrontierJoinMismatch(id(
                4,
                BlockId::new,
            )))
        };
        assert_eq!(
            validate_module(&module).map(|_| ()),
            expected,
            "{multiplicity:?}"
        );
    }
}

#[test]
fn a_consumed_affine_value_cannot_repair_twice() {
    let mut module = window_module();
    let operations = &mut module.machines[0].blocks[0].operations;
    // The moved result is affine now; the repair consumes it exactly once.
    let OperationResult::Structural(result) = &mut operations[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let mut second = repair(2);
    second.id = id(4, OperationId::new);
    operations.push(second);
    // The first store closes the debt and consumes the affine value; the
    // second finds neither a live value nor an open window.
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(4, OperationId::new),
            place: id(2, PlaceId::new),
        },
    );
}

#[test]
fn a_repair_to_a_closed_root_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3, OperationId::new),
            structural_type: cell(),
        },
    });
    // The first store has already reseated `left`; a second store to the same
    // field is a replacement-with-cleanup claim this operation never carries.
    machine.blocks[0].operations.insert(
        2,
        fresh_cell(id(3, OperationId::new), 3, id(1, ValueId::new)),
    );
    machine.blocks[0].operations.insert(
        0,
        boolean_constant(id(5, OperationId::new), id(1, ValueId::new), true),
    );
    let mut second = repair(3);
    second.id = id(4, OperationId::new);
    machine.blocks[0].operations.push(second);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::BorrowedStorageRepairMismatch {
            operation: id(4, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn crash_abandons_the_window_without_cleanup() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.pop();
    machine.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    }];
    machine.blocks[0].terminator = Terminator::Crash {
        edge: id(2, EdgeId::new),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("crash retains no restoration obligation");
}

#[test]
fn successor_arity_reports_before_window_debt() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    // Leave the `left` window open, then jump to a target declaring one
    // shared-borrow parameter while the edge carries two arguments that both
    // overlap the open window. Successor-argument arity reports before the
    // restoration-debt scan — a misordered scan would surface the window
    // violation instead.
    machine.blocks[0].operations.pop();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(2, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![
            StructuralArgument {
                place: id(1, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            StructuralArgument {
                place: id(1, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
        ],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(5, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(2, BlockId::new),
            position: 0,
        },
    });
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(5, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(3, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    });
    assert_rejects(
        &module,
        ModuleError::StructuralJumpArityMismatch {
            edge: id(2, EdgeId::new),
            expected: 1,
            actual: 2,
        },
    );
}

#[test]
fn overlapping_successor_arguments_report_in_edge_order() {
    for (first, second, reported) in [(3_u64, 1_u64, 3_u64), (1_u64, 3_u64, 1_u64)] {
        let mut module = window_module();
        let machine = &mut module.machines[0];
        machine.blocks[0].operations.pop();
        let mut second_parameter = machine.structural_parameters[0].clone();
        second_parameter.place = id(3, PlaceId::new);
        second_parameter.position = 1;
        machine.structural_parameters.push(second_parameter);
        let mut second_extraction = extract(id(3, OperationId::new), 4);
        let OperationKind::MoveStructuralField { source, .. } = &mut second_extraction.kind else {
            unreachable!()
        };
        *source = id(3, PlaceId::new);
        machine.blocks[0].operations.push(second_extraction);
        machine.structural_places.extend([
            StructuralPlaceDeclaration {
                id: id(3, PlaceId::new),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: id(4, PlaceId::new),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id(3, OperationId::new),
                    structural_type: cell(),
                },
            },
            StructuralPlaceDeclaration {
                id: id(5, PlaceId::new),
                kind: StructuralPlaceKind::BlockParameter {
                    block: id(2, BlockId::new),
                    position: 0,
                },
            },
            StructuralPlaceDeclaration {
                id: id(6, PlaceId::new),
                kind: StructuralPlaceKind::BlockParameter {
                    block: id(2, BlockId::new),
                    position: 1,
                },
            },
        ]);
        // Each borrowed envelope has its own open hole. The extracted values
        // are fresh owned storage, not aliases of those holes; only the two
        // incomplete borrowed roots are invalid successor arguments here.
        let argument = |place: u64| StructuralArgument {
            place: id(place, PlaceId::new),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        };
        machine.blocks[0].terminator = Terminator::Jump {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![argument(first), argument(second)],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: vec![
                StructuralParameterDeclaration {
                    place: id(5, PlaceId::new),
                    position: 0,
                    is_self: false,
                    structural_type: envelope(),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                StructuralParameterDeclaration {
                    place: id(6, PlaceId::new),
                    position: 1,
                    is_self: false,
                    structural_type: envelope(),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            ],
            id: id(2, BlockId::new),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(3, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        });
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidStructuralSuccessorArgument {
                edge: id(2, EdgeId::new),
                place: id(reported, PlaceId::new),
            },
            "edge order {first} then {second} must report place {reported}"
        );
    }
}

#[test]
fn a_conditional_reports_the_true_arm_successor_first() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.pop();
    machine.blocks[0].operations.push(boolean_constant(
        id(5, OperationId::new),
        id(1, ValueId::new),
        true,
    ));
    // Both arms carry the same window-overlapping argument into equivalent
    // targets; the true arm's edge must report first.
    let overlapping_argument = StructuralArgument {
        place: id(1, PlaceId::new),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(1, ValueId::new),
        when_true: SuccessorEdge {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![overlapping_argument.clone()],
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(3, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![overlapping_argument],
            trivial_affine_discards: Vec::new(),
        },
    };
    for (block, place) in [(id(2, BlockId::new), 5_u64), (id(3, BlockId::new), 6_u64)] {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(place, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
        });
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: id(place, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: envelope(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            id: block,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(10 + place, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(2, EdgeId::new),
            place: id(1, PlaceId::new),
        },
    );
}
