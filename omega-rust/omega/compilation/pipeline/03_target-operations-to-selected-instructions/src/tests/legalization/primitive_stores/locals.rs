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
