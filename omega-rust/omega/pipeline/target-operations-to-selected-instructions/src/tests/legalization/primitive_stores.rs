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
    TargetOperation, TargetOperationPlan, TargetUnitOperation,
    TargetUnitWriteOnlyPrimitiveStoreSource,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::{legalize_target_operations, validate_legalized_operations};

mod scalar_returns;

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
    source.structural_types.push(StructuralTypeDeclaration {
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
        psi_operation: OperationId::new(2).unwrap(),
        destination,
        value: AbstractResult {
            value,
            scalar_type: scalar,
        },
    });
    operations.append(&mut source.functions[0].operations);
    source.functions[0].operations = operations;
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

fn target_store(target: &mut TargetOperationPlan) -> &mut TargetUnitOperation {
    let TargetOperation::UnitBody(body) = &mut target.functions[0].operation else {
        panic!("Unit store body");
    };
    body.operations
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
