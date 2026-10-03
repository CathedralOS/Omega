use super::{
    AdmissionProfile, BlockId, IntegerSign, IntegerType, ModuleError, Operation, OperationId,
    OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, TerminalModule, ValueDeclaration, ValueId, id, ranked_countdown_proof,
    ranked_countdown_with_width, unranked_scalar_cycle, validate_module,
    validate_module_for_interpretation, verify_module, verify_module_for_optimization,
};
use semantic_vocabulary::StructuralFieldId;
use terminal_psi::{
    BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
};

fn scalar_field_declaration(
    field: u64,
    identity: &str,
    scalar_type: ScalarType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(field, StructuralFieldId::new),
        identity: identity.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(scalar_type),
    }
}

fn scalar_field_initializer(field: u64, value: u64) -> RecordFieldInitializer {
    RecordFieldInitializer {
        field: id(field, StructuralFieldId::new),
        value: RecordFieldValue::Scalar {
            value: id(value, ValueId::new),
            range_obligation: None,
        },
    }
}

fn record_result(place: u64, structural_type: StructuralTypeId) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: id(place, PlaceId::new),
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn record_place(
    place: u64,
    producer: u64,
    structural_type: StructuralTypeId,
) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: id(place, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(producer, OperationId::new),
            structural_type,
        },
    }
}

/// The unrestricted `Pair { first: u64, second: bool }` declaration the cyclic
/// establishment fixtures share.
fn declare_pair(module: &mut TerminalModule) -> StructuralTypeId {
    let pair = id(1, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: pair,
        identity: "test::Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                scalar_field_declaration(
                    1,
                    "first",
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                ),
                scalar_field_declaration(2, "second", ScalarType::Boolean),
            ],
        },
    });
    pair
}

fn record_establishment(
    operation: u64,
    place: u64,
    structural_type: StructuralTypeId,
    fields: Vec<RecordFieldInitializer>,
) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: record_result(place, structural_type),
        kind: OperationKind::EstablishRecord { fields },
    }
}

/// An unranked self-loop whose entry block additionally establishes a complete
/// unrestricted record each iteration: `pair` binds the machine's `seed`
/// parameter and the loop's `condition` parameter. Every field is scalar, so
/// the establishment moves no custody.
fn scalar_record_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let pair = declare_pair(&mut module);
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    });
    machine.structural_places.push(record_place(5, 20, pair));
    machine.blocks[0].operations.push(record_establishment(
        20,
        5,
        pair,
        vec![
            scalar_field_initializer(1, 20),
            scalar_field_initializer(2, 10),
        ],
    ));
    module
}

fn zeroed_record_array_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let pair = declare_pair(&mut module);
    let array = id(2, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: "test::Pairs".into(),
        shape: StructuralTypeShape::FixedArray {
            element: pair,
            length: 3,
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(record_place(5, 20, array));
    let mut establishment = record_establishment(20, 5, array, Vec::new());
    establishment.kind = OperationKind::EstablishZeroedStructural {};
    machine.blocks[0].operations.push(establishment);
    module
}

#[test]
fn cyclic_plain_zero_initialization_needs_no_disposal_obligation() {
    let mut module = zeroed_record_array_cycle();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("zero-initialized plain storage may be re-established each iteration");
    // The same operation also composes in an acyclic exit of a cyclic machine.
    let establishment = module.machines[0].blocks[0].operations.pop().unwrap();
    module.machines[0].blocks[1].operations.push(establishment);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a plain local in the exit inherits no loop disposal obligation");
}

#[test]
fn cyclic_zero_initialization_does_not_erase_affine_disposal() {
    let mut module = zeroed_record_array_cycle();
    let OperationResult::Structural(result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        unreachable!();
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ControlCycleIneligible { .. })
    ));
}

#[test]
fn cyclic_boundary_call_reborrows_a_produced_home_without_transferring_it() {
    let mut module = zeroed_record_array_cycle();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ));
    let boundary = id(1, semantic_vocabulary::BoundaryMachineId::new);
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            id: boundary,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            identity: "test::Pairs::update".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
                place: id(100, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: id(2, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(21, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(5, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    });
    validate_module(&module).expect("boundary and internal calls share call-scoped result loans");
    let mut unavailable = module.clone();
    unavailable.machines[0].blocks[0].operations.swap(0, 1);
    assert!(
        validate_module(&unavailable).is_err(),
        "the local must exist before lending it"
    );
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!();
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(
        validate_module(&module).is_err(),
        "a shared loan cannot satisfy mutable access"
    );
}

#[test]
fn cyclic_scalar_record_establishment_is_valid_under_every_authority() {
    let module = scalar_record_cycle();
    validate_module(&module).expect("an unrestricted record may re-establish each iteration");
    validate_module_for_interpretation(&module).expect("the record cycle is interpreter-valid");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the record cycle carries no obligations");
    verify_module_for_optimization(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the record cycle is optimizer-admissible");
}

#[test]
fn ranked_cycle_admits_a_member_record_establishment() {
    let mut module = ranked_countdown_with_width(64);
    let pair = declare_pair(&mut module);
    let machine = &mut module.machines[0];
    machine.structural_places.push(record_place(5, 20, pair));
    // The decrement member constructs a record from its own `next` result and
    // the header's `condition` — both ordinary scalar definitions.
    machine.blocks[2].operations.push(record_establishment(
        20,
        5,
        pair,
        vec![
            scalar_field_initializer(1, 6),
            scalar_field_initializer(2, 4),
        ],
    ));
    validate_module(&module).expect("a member record establishment keeps the ranked cycle valid");
    let proof = ranked_countdown_proof(&module);
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("the ranked record cycle verifies under full evidence");
    verify_module_for_optimization(&module, &proof, &AdmissionProfile::default())
        .expect("the ranked record cycle is optimizer-admissible");
}

/// A record whose declared field is itself structural copies an unrestricted
/// owned source: `Holder { inner: Pair }` binding the earlier `pair` place.
/// Copying an unrestricted root each iteration still moves no custody.
#[test]
fn cyclic_record_structural_field_copies_an_unrestricted_member_place() {
    let mut module = scalar_record_cycle();
    let pair = id(1, StructuralTypeId::new);
    let holder = id(2, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: holder,
        identity: "test::Holder".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(3, StructuralFieldId::new),
                identity: "inner".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(pair),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(record_place(6, 21, holder));
    machine.blocks[0].operations.push(record_establishment(
        21,
        6,
        holder,
        vec![RecordFieldInitializer {
            field: id(3, StructuralFieldId::new),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: id(5, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        }],
    ));
    validate_module(&module).expect("an unrestricted record copy stays custody-quiet in a cycle");
}

fn affine_record_cycle() -> TerminalModule {
    let mut module = scalar_record_cycle();
    let machine = &mut module.machines[0];
    let OperationResult::Structural(result) = &mut machine.blocks[0].operations[0].result else {
        panic!("the establishment is structural")
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let terminal_psi::Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("the fixture selects re-entry or normal return")
    };
    when_true.trivial_affine_discards = vec![id(5, PlaceId::new)];
    when_false.trivial_affine_discards = vec![id(5, PlaceId::new)];
    module
}

fn nested_affine_record_cycle() -> TerminalModule {
    let mut module = affine_record_cycle();
    let holder = id(2, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: holder,
        identity: "test::OwnedHolder".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(3, StructuralFieldId::new),
                identity: "inner".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(id(1, StructuralTypeId::new)),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(record_place(6, 21, holder));
    let mut operation = record_establishment(
        21,
        6,
        holder,
        vec![RecordFieldInitializer {
            field: id(3, StructuralFieldId::new),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: id(5, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        }],
    );
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    machine.blocks[0].operations.push(operation);
    let terminal_psi::Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards = vec![id(6, PlaceId::new)];
    when_false.trivial_affine_discards = vec![id(6, PlaceId::new)];
    module
}

#[test]
fn cyclic_affine_record_with_fields_rearms_after_selected_cleanup() {
    for module in [affine_record_cycle(), nested_affine_record_cycle()] {
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("complete affine records rearm only after exact cleanup");
        verify_module_for_optimization(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("ordinary cyclic custody remains optimizer admissible");
    }
}

#[test]
fn cyclic_affine_record_missing_backedge_cleanup_rejects_at_frontier() {
    let mut module = affine_record_cycle();
    let terminal_psi::Terminator::Conditional { when_true, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards.clear();
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(id(
            1,
            BlockId::new
        )))
    );
}

#[test]
fn cyclic_affine_record_cannot_dispose_a_child_consumed_by_its_parent() {
    let mut module = nested_affine_record_cycle();
    let terminal_psi::Terminator::Conditional { when_true, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards.push(id(5, PlaceId::new));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::EdgeAffineDiscardsInvalid {
            edge: id(10, semantic_vocabulary::EdgeId::new),
        })
    );
}

#[test]
fn cyclic_affine_record_cannot_reuse_a_consumed_child() {
    let mut module = nested_affine_record_cycle();
    let machine = &mut module.machines[0];
    let mut second_parent = machine.blocks[0].operations[1].clone();
    second_parent.id = id(22, OperationId::new);
    let OperationResult::Structural(result) = &mut second_parent.result else {
        unreachable!()
    };
    result.place = id(7, PlaceId::new);
    machine
        .structural_places
        .push(record_place(7, 22, result.structural_type));
    machine.blocks[0].operations.push(second_parent);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(22, OperationId::new),
            place: id(5, PlaceId::new),
        })
    );
}

/// `record::fields` runs during ordinary per-operation validation before the
/// cyclic-eligibility gate: a record whose `OperationResult` place was never
/// declared reports the pairing mismatch directly.
#[test]
fn cyclic_record_without_result_place_still_fails_pairing() {
    let mut module = scalar_record_cycle();
    module.machines[0].structural_places.clear();
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::RecordResultMismatch(id(20, OperationId::new)))
    );
}

/// A structural field initializer over a scalar-typed declaration mismatches
/// `record::fields` in the ordinary operand pass — the rejection stays
/// `RecordResultMismatch` even though the operation sits in a cycle.
#[test]
fn cyclic_record_structural_field_over_scalar_declaration_still_fails_pairing() {
    let mut module = scalar_record_cycle();
    let Operation {
        kind: OperationKind::EstablishRecord { fields },
        ..
    } = &mut module.machines[0].blocks[0].operations[0]
    else {
        panic!("the establishment is a record")
    };
    fields[1].value = RecordFieldValue::Structural(StructuralArgument {
        place: id(5, PlaceId::new),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::RecordResultMismatch(id(20, OperationId::new)))
    );
}

/// Eligibility only gates the family: a scalar field naming a value nothing
/// defines still rejects in the ordinary operand checks.
#[test]
fn cyclic_record_field_must_name_a_defined_scalar() {
    let mut module = scalar_record_cycle();
    let Operation {
        kind: OperationKind::EstablishRecord { fields },
        ..
    } = &mut module.machines[0].blocks[0].operations[0]
    else {
        panic!("the establishment is a record")
    };
    fields[0] = scalar_field_initializer(1, 77);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(77, ValueId::new)))
    );
}

#[path = "unranked_records/affine_home_loans.rs"]
mod affine_home_loans;
