//! Public legalization and independent replay custody for whole primitive stores.
use abstract_operations::{
    AbstractOperation, AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use legalized_operations::{LegalizedOperationPlan, LegalizedScalarInstructionKind};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetOperationPlan, TargetUnitOperation, TargetUnitWriteOnlyPrimitiveStoreSource,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::{legalize_target_operations, validate_legalized_operations};

mod block_values;
mod field_reads;
mod locals;
mod scalar_returns;

#[test]
fn indexed_primitive_storage_retains_root_path_footprint_and_access() {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar in [
            integer(IntegerSign::Unsigned, 8),
            integer(IntegerSign::Signed, 32),
        ] {
            for multiplicity in [
                StructuralMultiplicity::Unrestricted,
                StructuralMultiplicity::Affine,
            ] {
                let (mut source, _, _) = fixture(native, scalar, true);
                let root = source.structural_types[0].id;
                let leaf = StructuralTypeId::new(2).unwrap();
                let inner = StructuralTypeId::new(3).unwrap();
                let outer = StructuralTypeId::new(4).unwrap();
                let field = StructuralFieldId::new(2).unwrap();
                source.structural_types = vec![
                    StructuralTypeDeclaration {
                        id: root,
                        identity: "Root".into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![
                                StructuralFieldDeclaration {
                                    id: StructuralFieldId::new(1).unwrap(),
                                    identity: "padding".into(),
                                    relevance: BindingRelevance::Relevant,
                                    field_type: StructuralFieldType::Scalar(scalar),
                                },
                                StructuralFieldDeclaration {
                                    id: field,
                                    identity: "elements".into(),
                                    relevance: BindingRelevance::Relevant,
                                    field_type: StructuralFieldType::Structural(outer),
                                },
                            ],
                        },
                    },
                    StructuralTypeDeclaration {
                        id: leaf,
                        identity: "Element".into(),
                        shape: StructuralTypeShape::PrimitiveScalar(scalar),
                    },
                    StructuralTypeDeclaration {
                        id: inner,
                        identity: "Inner".into(),
                        shape: StructuralTypeShape::FixedArray {
                            element: leaf,
                            length: 3,
                        },
                    },
                    StructuralTypeDeclaration {
                        id: outer,
                        identity: "Outer".into(),
                        shape: StructuralTypeShape::FixedArray {
                            element: inner,
                            length: 2,
                        },
                    },
                ]
                .into();
                let path = vec![
                    Segment::Field(field),
                    Segment::FixedIndex(1),
                    Segment::FixedIndex(2),
                ];
                let function = &mut source.functions[0];
                function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
                function.structural_parameters[0].multiplicity = multiplicity;
                let parameter = function.structural_parameters[0].clone();
                let mut store = function.operations[1].clone();
                let AbstractOperation::WriteOnlyPrimitiveStore {
                    destination,
                    path: stored_path,
                    ..
                } = &mut store
                else {
                    panic!("store");
                };
                *destination = parameter.clone();
                *stored_path = path.clone();
                let read = AbstractResult {
                    value: ValueId::new(7).unwrap(),
                    scalar_type: scalar,
                };
                let result = AbstractResult {
                    value: ValueId::new(8).unwrap(),
                    scalar_type: scalar,
                };
                function.result = abstract_operations::AbstractFunctionResult::Scalar(result);
                function.operations = vec![
                    function.operations[0].clone(),
                    store,
                    AbstractOperation::PrimitiveScalarRead {
                        psi_operation: OperationId::new(3).unwrap(),
                        result: read,
                        source: parameter.place,
                        path: path.clone(),
                    },
                    AbstractOperation::Return {
                        psi_edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                        result: result.value,
                        value: read.value,
                        scalar_type: scalar,
                        cleanup_actions: Vec::new(),
                    },
                ];
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
                validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                    .unwrap();
                let width =
                    crate::structural_inputs::structural_reference_input::scalar_shape(scalar)
                        .unwrap()
                        .byte_size;
                let expected_offset = u32::from(width) * 6;
                let mut inspect = legalized.plan().clone();
                assert!(
                    matches!(legalized_store(&mut inspect).kind, LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { byte_offset, byte_size, .. } if byte_offset == expected_offset && u16::from(byte_size) == width)
                );
                assert_eq!(
                    legalized.plan().scalar_functions[0].call_plan.parameters[0]
                        .shape
                        .byte_size,
                    width * 7
                );
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
                for mutation in 0..5 {
                    let mut changed = target.clone();
                    let TargetUnitOperation::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        destination_type,
                        destination_placement,
                        ..
                    } = target_store(&mut changed)
                    else {
                        panic!("target store");
                    };
                    match mutation {
                        0 => path[2] = Segment::FixedIndex(1),
                        1 => path[2] = Segment::FixedIndex(3),
                        2 => destination.place = PlaceId::new(99).unwrap(),
                        3 => destination_type.id = leaf,
                        _ => destination_placement.shape.byte_size = width,
                    }
                    reject_target(&source, &changed, &unit, legalized.plan());
                }
                for mutation in 0..5 {
                    let mut changed = legalized.plan().clone();
                    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        byte_offset,
                        byte_size,
                        ..
                    } = &mut legalized_store(&mut changed).kind
                    else {
                        panic!("legal store");
                    };
                    match mutation {
                        0 => *byte_offset = 0,
                        1 => *byte_size = if width == 1 { 4 } else { 1 },
                        2 => path[2] = Segment::FixedIndex(1),
                        3 => destination.access = StructuralAccess::SharedBorrow,
                        _ => destination.structural_type = leaf,
                    }
                    assert!(
                        validate_legalized_operations(&target, &source, &unit, changed).is_err()
                    );
                }
                for changed_path in [
                    vec![],
                    vec![
                        Segment::Field(field),
                        Segment::FixedIndex(1),
                        Segment::FixedIndex(1),
                    ],
                ] {
                    let mut changed = legalized.plan().clone();
                    let LegalizedScalarInstructionKind::PrimitiveScalarRead { path, .. } =
                        &mut changed.scalar_functions[0].blocks[0].instructions[2].kind
                    else {
                        panic!("read");
                    };
                    *path = changed_path;
                    assert!(
                        validate_legalized_operations(&target, &source, &unit, changed).is_err()
                    );
                }
                let mut changed = selected.plan().clone();
                let instruction = changed.functions[0].blocks[0]
                    .instructions
                    .iter_mut()
                    .find(|row| {
                        matches!(
                            row.kind,
                            selected_instructions::SelectedInstructionKind::Store { .. }
                        )
                    })
                    .unwrap();
                let selected_instructions::SelectedInstructionKind::Store { byte_offset, .. } =
                    &mut instruction.kind
                else {
                    unreachable!()
                };
                *byte_offset = 0;
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err()
                );
                for access in [
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::WriteOnlyBorrow,
                ] {
                    let mut changed = source.clone();
                    changed.functions[0].structural_parameters[0].access = access;
                    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
                        &mut changed.functions[0].operations[1]
                    else {
                        panic!("store");
                    };
                    destination.access = access;
                    assert!(
                        abstract_operations_to_target_operations::lower_to_target_operations(
                            &changed,
                            abstract_operations_to_target_operations::TargetLoweringRequest::new(
                                native
                            )
                        )
                        .is_err()
                    );
                }
                let mut linear = source.clone();
                linear.functions[0].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Linear;
                let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
                    &mut linear.functions[0].operations[1]
                else {
                    panic!("store");
                };
                destination.multiplicity = StructuralMultiplicity::Linear;
                assert!(
                    abstract_operations_to_target_operations::lower_to_target_operations(
                        &linear,
                        abstract_operations_to_target_operations::TargetLoweringRequest::new(
                            native
                        )
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn multiple_record_inputs_keep_exact_field_store_destination_through_replay() {
    let scalar = integer(IntegerSign::Signed, 32);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (mut source, _, _) = fixture(native, scalar, true);
        let field = StructuralFieldId::new(1).unwrap();
        source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(scalar),
            }],
        };
        let mut other = source.functions[0].structural_parameters[0].clone();
        other.place = PlaceId::new(2).unwrap();
        other.position = 1;
        // A second independently writable record is a valid store destination
        // in isolation, but cannot replace this occurrence's selected target.
        other.access = StructuralAccess::MutableBorrow;
        source.functions[0]
            .structural_parameters
            .push(other.clone());
        let store = source.functions[0]
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
            .unwrap();
        let AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            value,
            ..
        } = store
        else {
            unreachable!()
        };
        *store = AbstractOperation::StructuralScalarFieldStore {
            range_obligation: None,
            psi_operation: *psi_operation,
            destination: destination.clone(),
            value: *value,
            path: Vec::new(),
            field,
        };
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
        let mut changed = target.clone();
        let store = changed.functions[0].graph.blocks[0]
            .operations
            .iter_mut()
            .find_map(|operation| {
                if let TargetUnitOperation::StructuralScalarFieldStore { destination, .. } =
                    operation
                {
                    Some(destination)
                } else {
                    None
                }
            })
            .unwrap();
        *store = other.clone();
        reject_target(&source, &changed, &unit, &legalized);
        let mut changed = legalized;
        let store = changed.scalar_functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find_map(|row| {
                if let LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination,
                    ..
                } = &mut row.kind
                {
                    Some(destination)
                } else {
                    None
                }
            })
            .unwrap();
        *store = other;
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
    }
}

fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
}

fn fixture(
    native: NativeTarget,
    scalar: ScalarType,
    literal: bool,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let value = ValueId::new(5).unwrap();
    let destination = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "Primitive".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        });
    source.functions[0]
        .structural_parameters
        .push(destination.clone());
    let mut operations = Vec::new();
    if literal {
        operations.push(match scalar {
            ScalarType::Boolean => AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: value,
                value: true,
            },
            ScalarType::Integer(carrier) => AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: value,
                scalar_type: scalar,
                value: if carrier.sign() == IntegerSign::Signed {
                    IntegerValue::Signed(-7)
                } else {
                    IntegerValue::Unsigned(17)
                },
            },
            _ => panic!("fixed primitive fixture"),
        });
    } else {
        source.functions[0].parameters = vec![
            AbstractParameter {
                value,
                scalar_type: scalar,
            },
            AbstractParameter {
                value: ValueId::new(6).unwrap(),
                scalar_type: scalar,
            },
        ];
    }
    operations.push(AbstractOperation::WriteOnlyPrimitiveStore {
        path: Vec::new(),
        psi_operation: OperationId::new(2).unwrap(),
        destination,
        value: AbstractResult {
            value,
            scalar_type: scalar,
        },
    });
    operations.append(&mut source.functions[0].operations);
    source.functions[0].operations = operations;
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
    (source, target, unit)
}

fn target_store(target: &mut TargetOperationPlan) -> &mut TargetUnitOperation {
    let body = &mut target.functions[0].graph;
    body.blocks[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
            )
        })
        .unwrap()
}

fn legalized_store(
    plan: &mut LegalizedOperationPlan,
) -> &mut legalized_operations::LegalizedScalarInstruction {
    plan.scalar_functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find(|row| {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .unwrap()
}

fn reject_target(
    source: &AbstractOperationPlan,
    target: &TargetOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) {
    assert!(legalize_target_operations(target, source, unit).is_err());
    assert!(validate_legalized_operations(target, source, unit, proposed.clone()).is_err());
}

#[test]
fn primitive_store_widths_and_boolean_sources_have_distinct_exact_custody() {
    let mut scalars = vec![ScalarType::Boolean];
    for bits in [8, 16, 32, 64] {
        scalars.push(integer(IntegerSign::Signed, bits));
        scalars.push(integer(IntegerSign::Unsigned, bits));
    }
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar in &scalars {
            for literal in [false, true] {
                let (source, target, unit) = fixture(native, *scalar, literal);
                let legal = legalize_target_operations(&target, &source, &unit).unwrap();
                validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
                    .unwrap();
                let mut plan = legal.plan().clone();
                let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    value,
                    byte_size,
                    ..
                } = &legalized_store(&mut plan).kind
                else {
                    unreachable!()
                };
                assert_eq!(value.scalar_type, *scalar);
                let expected = match scalar {
                    ScalarType::Boolean => 1,
                    ScalarType::Integer(carrier) => carrier.bits() / 8,
                    _ => unreachable!(),
                };
                assert_eq!(u16::from(*byte_size), expected);
            }
        }
    }
}

#[test]
fn target_store_rejects_changed_destination_type_parameter_and_ssa_identity() {
    let scalar = integer(IntegerSign::Signed, 32);
    let (source, target, unit) = fixture(NativeTarget::linux_x64(), scalar, false);
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in 0..11 {
        let mut changed = target.clone();
        let TargetUnitOperation::WriteOnlyPrimitiveStore {
            destination,
            destination_type,
            destination_placement,
            source: supplied,
            ..
        } = target_store(&mut changed)
        else {
            unreachable!()
        };
        match mutation {
            0 => destination.place = PlaceId::new(2).unwrap(),
            1 => destination.structural_type = StructuralTypeId::new(2).unwrap(),
            2 => destination.access = StructuralAccess::SharedBorrow,
            3 => destination_type.id = StructuralTypeId::new(2).unwrap(),
            4 => {
                destination_type.shape =
                    StructuralTypeShape::PrimitiveScalar(integer(IntegerSign::Unsigned, 32))
            }
            5 => {
                destination_type.shape = StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "not_a_primitive".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    }],
                }
            }
            6 => destination_placement.shape.byte_size = 8,
            7 => {
                let TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                    parameter_index, ..
                } = supplied
                else {
                    unreachable!()
                };
                *parameter_index = 1;
            }
            8 => {
                let TargetUnitWriteOnlyPrimitiveStoreSource::Parameter { source_value, .. } =
                    supplied
                else {
                    unreachable!()
                };
                *source_value = ValueId::new(6).unwrap();
            }
            9 => {
                let TargetUnitWriteOnlyPrimitiveStoreSource::Parameter { scalar_type, .. } =
                    supplied
                else {
                    unreachable!()
                };
                *scalar_type = integer(IntegerSign::Unsigned, 32);
            }
            10 => {
                let TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                    parameter_index,
                    source_value,
                    ..
                } = supplied
                else {
                    unreachable!()
                };
                *parameter_index = 1;
                *source_value = ValueId::new(6).unwrap();
            }
            _ => unreachable!(),
        }
        reject_target(&source, &changed, &unit, legal.plan());
    }
}

#[test]
fn target_literal_store_rejects_changed_bits_definition_and_boolean_integer_impostor() {
    for scalar in [
        ScalarType::Boolean,
        integer(IntegerSign::Signed, 32),
        integer(IntegerSign::Unsigned, 64),
    ] {
        let (source, target, unit) = fixture(NativeTarget::linux_arm64(), scalar, true);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..4 {
            let mut changed = target.clone();
            let TargetUnitOperation::WriteOnlyPrimitiveStore {
                source: supplied, ..
            } = target_store(&mut changed)
            else {
                unreachable!()
            };
            match supplied {
                TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    value,
                    ..
                } => match mutation {
                    0 => *defining_operation = OperationId::new(9).unwrap(),
                    1 => *source_value = ValueId::new(9).unwrap(),
                    2 => *value = IntegerValue::Unsigned(18),
                    _ => {
                        *supplied = TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                            defining_operation: *defining_operation,
                            source_value: *source_value,
                            value: true,
                        }
                    }
                },
                TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                } => match mutation {
                    0 => *defining_operation = OperationId::new(9).unwrap(),
                    1 => *source_value = ValueId::new(9).unwrap(),
                    2 => *value = false,
                    _ => {
                        *supplied = TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                            defining_operation: *defining_operation,
                            source_value: *source_value,
                            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                            value: IntegerValue::Unsigned(1),
                        }
                    }
                },
                _ => panic!("literal store source"),
            }
            reject_target(&source, &changed, &unit, legal.plan());
        }
    }
}

#[test]
fn independent_replay_rejects_width_value_destination_kind_and_fuel_substitution() {
    for scalar in [ScalarType::Boolean, integer(IntegerSign::Signed, 32)] {
        let (source, target, unit) = fixture(NativeTarget::linux_x64(), scalar, false);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..9 {
            let mut changed = legal.plan().clone();
            let row = legalized_store(&mut changed);
            let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                destination,
                value,
                byte_size,
                ..
            } = &mut row.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *byte_size = if *byte_size == 1 { 2 } else { 1 },
                1 => value.value = ValueId::new(6).unwrap(),
                2 => {
                    value.scalar_type = if scalar == ScalarType::Boolean {
                        integer(IntegerSign::Unsigned, 8)
                    } else {
                        integer(IntegerSign::Unsigned, 32)
                    }
                }
                3 => destination.place = PlaceId::new(2).unwrap(),
                4 => destination.access = StructuralAccess::Owned,
                5 => destination.structural_type = StructuralTypeId::new(2).unwrap(),
                6 => {
                    row.kind = LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                        destination: destination.clone(),
                        path: Vec::new(),
                        field: StructuralFieldId::new(1).unwrap(),
                        value: *value,
                        byte_offset: 0,
                        byte_size: *byte_size,
                    }
                }
                7 => row.fuel.clear(),
                8 => row.effect.output += 1,
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
