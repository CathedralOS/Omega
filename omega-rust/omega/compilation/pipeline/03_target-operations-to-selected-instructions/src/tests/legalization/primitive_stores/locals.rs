//! Receiving replay binds primitive local operations to exact source identities.
use super::{
    AbstractOperation, AbstractOperationPlan, AbstractResult, FuelScheduleIdentity, IntegerSign,
    IntegerValue, LegalizedScalarInstructionKind, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TargetUnitOperation, ValueId,
};
use crate::legalize_target_operations;
use crate::tests::legalization::primitive_stores::integer;
use crate::validate_legalized_operations;

fn fixture(scalar: ScalarType) -> AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let identity = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: identity,
            identity: "primitive".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        });
    let returned = source.functions[0].operations.pop().unwrap();
    source.functions[0].operations = vec![
        if scalar == ScalarType::Boolean {
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: ValueId::new(1).unwrap(),
                value: true,
            }
        } else {
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: ValueId::new(1).unwrap(),
                scalar_type: scalar,
                value: IntegerValue::Unsigned(91),
            }
        },
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation: OperationId::new(2).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: identity,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            value: AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
        },
        AbstractOperation::PrimitiveScalarRead {
            path: Vec::new(),
            psi_operation: OperationId::new(3).unwrap(),
            source: place,
            result: AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type: scalar,
            },
        },
        AbstractOperation::PrimitiveLocalStore {
            psi_operation: OperationId::new(4).unwrap(),
            destination: place,
            value: AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type: scalar,
            },
        },
        returned,
    ];
    source
}

fn mixed_storage_fixture() -> AbstractOperationPlan {
    let scalar = integer(IntegerSign::Unsigned, 64);
    let mut source = fixture(scalar);
    let literal_type = StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: "literal bytes".into(),
        shape: StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
    };
    let literal = terminal_psi::StructuralPlaceDeclaration {
        id: PlaceId::new(2).unwrap(),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            // This is the first literal, although other operations and a
            // primitive storage declaration precede its establishment.
            declaration_ordinal: 0,
            structural_type: literal_type.id,
        },
    };
    source
        .structural_types
        .make_mut()
        .push(literal_type.clone());
    let function = &mut source.functions[0];
    function.operations.splice(
        2..3,
        [
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: OperationId::new(5).unwrap(),
                place: literal,
                structural_type: literal_type,
                bytes: vec![0, 0x80, 0xff],
                qualifications: Vec::new(),
            },
            AbstractOperation::ByteSequenceLength {
                psi_operation: OperationId::new(3).unwrap(),
                result: AbstractResult {
                    value: ValueId::new(2).unwrap(),
                    scalar_type: scalar,
                },
                source: literal.id,
            },
        ],
    );
    function.operations.insert(
        function.operations.len() - 1,
        AbstractOperation::PrimitiveScalarRead {
            path: Vec::new(),
            psi_operation: OperationId::new(6).unwrap(),
            source: PlaceId::new(1).unwrap(),
            result: AbstractResult {
                value: ValueId::new(3).unwrap(),
                scalar_type: scalar,
            },
        },
    );

    source
}

fn check_mixed_storage(source: &AbstractOperationPlan) {
    // Both homes reach lowering directly: no optional optimization can erase
    // the literal observation or the local's establishment, store and read.
    let native = NativeTarget::macos_arm64();
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .expect("mixed storage lowers through the ordinary target graph");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let legalized = legalize_target_operations(&target, source, &unit)
        .expect("unrelated storage producers compose in one graph");
    validate_legalized_operations(&target, source, &unit, legalized.plan().clone()).unwrap();
    assert_eq!(
        legalized.plan().scalar_functions[0].blocks.len(),
        source.functions[0].block_entries.len()
    );
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
    let constraints = crate::selection_constraints(&legalized, &environment);
    let selected = crate::select_instructions(
        &legalized,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("selection retains both ordinary storage homes");
    crate::validate_selected_instructions(
        &legalized,
        &constraints,
        environment.physical(),
        environment.constraints(),
        selected.plan().clone(),
    )
    .unwrap();

    let mut substituted = target.clone();
    let literal = substituted.functions[0]
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation,
                TargetUnitOperation::EstablishByteSequenceLiteral { .. }
            )
        })
        .unwrap();
    let TargetUnitOperation::EstablishByteSequenceLiteral { place, .. } = literal else {
        panic!("literal follows scalar computation and local establishment");
    };
    place.id = PlaceId::new(1).unwrap();
    assert!(legalize_target_operations(&substituted, source, &unit).is_err());
    assert!(
        validate_legalized_operations(&substituted, source, &unit, legalized.plan().clone())
            .is_err(),
        "mixed admission must still reject a literal substituted for the primitive home"
    );

    for mutation in 0..4 {
        let mut changed = unit.clone();
        let places = &mut changed.functions[0].structural_places;
        match mutation {
            0 => places.retain(|place| place.id != PlaceId::new(2).unwrap()),
            1 => {
                let declaration = places
                    .iter_mut()
                    .find(|place| place.id == PlaceId::new(1).unwrap())
                    .unwrap();
                let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
                    &mut declaration.kind
                else {
                    panic!("primitive local producer");
                };
                *producer = OperationId::new(5).unwrap();
            }
            2 => {
                let declaration = places
                    .iter_mut()
                    .find(|place| place.id == PlaceId::new(2).unwrap())
                    .unwrap();
                let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    ..
                } = &mut declaration.kind
                else {
                    panic!("literal declaration");
                };
                *declaration_ordinal = 1;
            }
            _ => {
                let mut undeclared = *places
                    .iter()
                    .find(|place| place.id == PlaceId::new(2).unwrap())
                    .unwrap();
                undeclared.id = PlaceId::new(99).unwrap();
                places.push(undeclared);
                changed.functions[0].declared_places.insert(undeclared.id);
            }
        }
        // Recompute the commitment so rejection must inspect custody rather
        // than merely detect a stale identity after the mutation.
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(
            legalize_target_operations(&target, source, &changed).is_err(),
            "malformed mixed storage roster {mutation}"
        );
        assert!(
            validate_legalized_operations(&target, source, &changed, legalized.plan().clone())
                .is_err(),
            "independent malformed storage replay {mutation}"
        );
    }

    let mut changed = selected.plan().clone();
    let materialization = changed.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| {
            instruction
                .provenance
                .operations
                .contains(&OperationId::new(5).unwrap())
                && matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::MaterializeI64 { .. }
                )
        })
        .expect("literal byte materialization");
    materialization.kind = selected_instructions::SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(0),
    };
    assert!(
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            changed,
        )
        .is_err(),
        "literal bytes must be reconstructed independently beside primitive storage"
    );
}

#[test]
fn primitive_local_and_byte_literal_compose_after_scalar_computation() {
    check_mixed_storage(&mixed_storage_fixture());
}

#[test]
fn primitive_local_and_byte_literal_compose_with_scalar_return() {
    let mut source = mixed_storage_fixture();
    let scalar = integer(IntegerSign::Unsigned, 64);
    let function = &mut source.functions[0];
    let AbstractOperation::ReturnUnit {
        psi_edge,
        cleanup_actions,
    } = function.operations.pop().unwrap()
    else {
        panic!("fixture Unit return");
    };
    function.result = abstract_operations::AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(4).unwrap(),
        scalar_type: scalar,
    });
    function.operations.push(AbstractOperation::Return {
        psi_edge,
        result: ValueId::new(4).unwrap(),
        value: ValueId::new(3).unwrap(),
        scalar_type: scalar,
        cleanup_actions,
    });
    check_mixed_storage(&source);
}

#[test]
fn primitive_local_and_byte_literal_compose_after_scalar_successor_binding() {
    let mut source = mixed_storage_fixture();
    let scalar = integer(IntegerSign::Unsigned, 64);
    let successor = semantic_vocabulary::BlockId::new(2).unwrap();
    let parameter = ValueId::new(4).unwrap();
    let function = &mut source.functions[0];
    // The scalar computation belongs to the predecessor; ordinary value
    // binding supplies the local initializer in the following block.
    function.operations.insert(
        1,
        AbstractOperation::Jump {
            structural_bindings: Vec::new(),
            psi_edge: semantic_vocabulary::EdgeId::new(2).unwrap(),
            target: successor,
            bindings: vec![abstract_operations::ValueBinding {
                parameter,
                argument: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    function
        .block_entries
        .push(abstract_operations::AbstractBlockEntry {
            block: successor,
            parameters: vec![abstract_operations::AbstractParameter {
                value: parameter,
                scalar_type: scalar,
            }],
            structural_parameters: Vec::new(),
            operation_offset: 2,
        });
    let AbstractOperation::EstablishPrimitiveLocal { value, .. } = &mut function.operations[2]
    else {
        panic!("local initializer follows the jump");
    };
    value.value = parameter;
    check_mixed_storage(&source);
}

#[test]
fn boolean_field_store_replays_the_exact_dominating_scalar_home() {
    let mut source = fixture(ScalarType::Boolean);
    let record = StructuralTypeId::new(2).unwrap();
    let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: record,
            identity: "BooleanDestination".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: field,
                    identity: "flag".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        });
    let destination = terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(2).unwrap(),
        position: 0,
        is_self: false,
        structural_type: record,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    source.functions[0]
        .structural_parameters
        .push(destination.clone());
    source.functions[0].operations[3] = AbstractOperation::StructuralScalarFieldStore {
        psi_operation: OperationId::new(4).unwrap(),
        destination: destination.into(),
        path: Vec::new(),
        field,
        value: AbstractResult {
            value: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
        },
        range_obligation: None,
    };
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        for mutation in 0..5 {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldStore {
                source: target_operations::TargetUnitScalarArgumentSource::Home(home),
                ..
            } = &mut changed.functions[0].graph.blocks[0].operations[3]
            else {
                panic!("field store keeps the read's scalar home");
            };
            match mutation {
                0 => home.defining_operation = OperationId::new(1).unwrap(),
                1 => home.source_value = ValueId::new(1).unwrap(),
                2 => home.scalar_type = integer(IntegerSign::Unsigned, 8),
                3 => home.shape.byte_size = 4,
                _ => changed.functions[0].graph.blocks[0].operations.swap(2, 3),
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target {native:?}, scalar home mutation {mutation}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone(),)
                    .is_err(),
                "independent replay, target {native:?}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn local_establishment_store_and_read_reject_target_and_legalized_corruption() {
    let source = fixture(integer(IntegerSign::Unsigned, 64));
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        for mutation in 0..5 {
            let mut changed = target.clone();
            let graph = &mut changed.functions[0].graph;
            match mutation {
                0 => {
                    let TargetUnitOperation::EstablishPrimitiveLocal { shape, .. } =
                        &mut graph.blocks[0].operations[1]
                    else {
                        panic!("local");
                    };
                    shape.byte_size = 4;
                }
                1 => {
                    let TargetUnitOperation::PrimitiveScalarRead { source, .. } =
                        &mut graph.blocks[0].operations[2]
                    else {
                        panic!("read");
                    };
                    *source = PlaceId::new(99).unwrap();
                }
                2 => {
                    let TargetUnitOperation::PrimitiveLocalStore { value, .. } =
                        &mut graph.blocks[0].operations[3]
                    else {
                        panic!("store");
                    };
                    value.value = ValueId::new(1).unwrap();
                }
                3 => graph.blocks[0].operations.swap(1, 2),
                _ => {
                    let TargetUnitOperation::PrimitiveScalarRead { result, .. } =
                        &mut graph.blocks[0].operations[2]
                    else {
                        panic!("read");
                    };
                    result.value = ValueId::new(1).unwrap();
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err(),
                "receiving mutation {mutation}"
            );
        }
        for mutation in 0..4 {
            let mut changed = legalized.plan().clone();
            let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
            match mutation {
                0 => rows[1].fuel.clear(),
                1 => rows[1].effect.output += 1,
                2 => {
                    let LegalizedScalarInstructionKind::PrimitiveLocalStore { value, .. } =
                        &mut rows[3].kind
                    else {
                        panic!("store");
                    };
                    value.value = ValueId::new(1).unwrap();
                }
                _ => rows[2].result.as_mut().unwrap().value = ValueId::new(1).unwrap(),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "legalized mutation {mutation}"
            );
        }
    }
}

#[test]
fn owned_record_field_store_rejoins_its_producer_and_home() {
    use abstract_operations::StructuralScalarStoreDestination;
    let mut source = fixture(ScalarType::Boolean);
    let record = StructuralTypeId::new(2).unwrap();
    let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
    let producer = OperationId::new(4).unwrap();
    let result = terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(2).unwrap(),
        structural_type: record,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: record,
            identity: "OwnedBooleanDestination".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: field,
                    identity: "flag".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        });
    source.functions[0].operations.splice(
        3..4,
        [
            AbstractOperation::EstablishRecord {
                psi_operation: producer,
                result: result.clone(),
                fields: vec![terminal_psi::RecordFieldInitializer {
                    field,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: ValueId::new(1).unwrap(),
                        range_obligation: None,
                    },
                }],
            },
            AbstractOperation::StructuralScalarFieldStore {
                psi_operation: OperationId::new(5).unwrap(),
                destination: StructuralScalarStoreDestination::OwnedResult { producer, result },
                path: Vec::new(),
                field,
                value: AbstractResult {
                    value: ValueId::new(2).unwrap(),
                    scalar_type: ScalarType::Boolean,
                },
                range_obligation: None,
            },
        ],
    );
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit)
            .unwrap()
            .plan()
            .clone();
        validate_legalized_operations(&target, &source, &unit, legalized.clone()).unwrap();
        for mutation in 0..2 {
            let mut changed = target.clone();
            let store = changed.functions[0].graph.blocks[0]
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::StructuralScalarFieldStore { .. }
                    )
                })
                .unwrap();
            let TargetUnitOperation::StructuralScalarFieldStore {
                destination: StructuralScalarStoreDestination::OwnedResult { producer, result },
                ..
            } = store
            else {
                panic!("owned destination");
            };
            if mutation == 0 {
                *producer = OperationId::new(2).unwrap();
            } else {
                result.place = PlaceId::new(1).unwrap();
            }
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.clone()).is_err()
            );

            let mut changed = legalized.clone();
            let store = changed.scalar_functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                    )
                })
                .unwrap();
            let LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                destination: StructuralScalarStoreDestination::OwnedResult { producer, result },
                ..
            } = &mut store.kind
            else {
                panic!("owned destination");
            };
            if mutation == 0 {
                *producer = OperationId::new(2).unwrap();
            } else {
                result.place = PlaceId::new(1).unwrap();
            }
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }
    }
}

#[test]
fn local_array_primitive_read_rejoins_original_home_and_projected_offset() {
    use terminal_psi::StructuralPathSegment;

    let scalar = integer(IntegerSign::Unsigned, 8);
    let mut source = fixture(scalar);
    let array = StructuralTypeId::new(2).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: array,
            identity: "LocalBytes".into(),
            shape: StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: 4,
            },
        });
    let function = &mut source.functions[0];
    let returned = function.operations.pop().unwrap();
    function.operations = vec![
        AbstractOperation::EstablishZeroedStructural {
            psi_operation: OperationId::new(1).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: PlaceId::new(1).unwrap(),
                structural_type: array,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        },
        AbstractOperation::PrimitiveScalarRead {
            psi_operation: OperationId::new(2).unwrap(),
            result: AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
            source: PlaceId::new(1).unwrap(),
            path: vec![StructuralPathSegment::FixedIndex(2)],
        },
        returned,
    ];
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();

        for mutation in 0..3 {
            let mut changed = legalized.plan().clone();
            let row = changed.scalar_functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        LegalizedScalarInstructionKind::PrimitiveScalarRead { .. }
                    )
                })
                .unwrap();
            let LegalizedScalarInstructionKind::PrimitiveScalarRead {
                source: read_source,
                path,
                ..
            } = &mut row.kind
            else {
                panic!("projected local read");
            };
            match mutation {
                0 => path[0] = StructuralPathSegment::FixedIndex(3),
                1 => *read_source = PlaceId::new(99).unwrap(),
                _ => row.result.as_mut().unwrap().scalar_type = integer(IntegerSign::Unsigned, 16),
            }
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }

        let mut changed = selected.plan().clone();
        let load = changed.functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| {
                row.provenance.operations == [OperationId::new(2).unwrap()]
                    && matches!(
                        row.kind,
                        selected_instructions::SelectedInstructionKind::Load8 { .. }
                    )
            })
            .expect("projected byte load");
        let selected_instructions::SelectedInstructionKind::Load8 { byte_offset } = &mut load.kind
        else {
            panic!("byte load");
        };
        *byte_offset += 1;
        assert!(
            crate::validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                changed,
            )
            .is_err(),
            "a neighboring byte cannot substitute for the retained element"
        );
    }
}
