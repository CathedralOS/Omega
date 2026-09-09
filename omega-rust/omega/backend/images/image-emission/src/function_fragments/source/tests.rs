use super::*;
use semantic_vocabulary::{IeeeFloatValue, OperationId, ValueId};
use target_operations::TargetUnitOperation;

#[test]
fn replay_requirement_is_specific_to_primitive_storage_operations() {
    use abstract_operations::AbstractResult;
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
        StructuralParameterDeclaration,
    };

    let operation = OperationId::new(1).unwrap();
    let scalar = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    };
    let result = StructuralOperationResult {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        claims: vec![],
    };
    let existing = [
        AbstractOperation::BooleanConstant {
            psi_operation: operation,
            result: scalar.value,
            value: true,
        },
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation: operation,
            destination: StructuralParameterDeclaration {
                place: result.place,
                position: 0,
                is_self: false,
                structural_type: result.structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: vec![],
                projected_qualifications: vec![],
            },
            value: scalar,
        },
    ];
    assert!(!requires_graph_storage_replay(&[]));
    assert!(!requires_graph_storage_replay(&existing));
    for operation in [
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation: operation,
            result: result.clone(),
            value: scalar,
        },
        AbstractOperation::PrimitiveLocalStore {
            psi_operation: operation,
            destination: result.place,
            value: scalar,
        },
        AbstractOperation::PrimitiveScalarRead {
            psi_operation: operation,
            result: scalar,
            source: result.place,
        },
    ] {
        assert!(requires_graph_storage_replay(std::slice::from_ref(
            &operation
        )));
        let mut mixed = existing.to_vec();
        mixed.push(operation);
        assert!(requires_graph_storage_replay(&mixed));
    }
}

#[test]
fn ieee_literal_publication_requires_unique_operation_value_type_and_bits() {
    let operation = OperationId::new(1).unwrap();
    let result = ValueId::new(1).unwrap();
    for value in [
        IeeeFloatValue::Binary32(0x7fc0_0041),
        IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
    ] {
        let source = AbstractOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        };
        let target = TargetUnitOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        };
        assert!(ieee_literal_retained(
            &source,
            std::slice::from_ref(&target)
        ));
        assert!(!ieee_literal_retained(&source, &[]));
        assert!(!ieee_literal_retained(
            &source,
            &[target.clone(), target.clone()]
        ));
        for mutation in 0..4 {
            let mut changed = target.clone();
            let TargetUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            } = &mut changed
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => *result = ValueId::new(2).unwrap(),
                2 => {
                    *value = match *value {
                        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary32(bits ^ 1),
                        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary64(bits ^ 1),
                    }
                }
                _ => {
                    *value = match *value {
                        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary64(u64::from(bits)),
                        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary32(bits as u32),
                    }
                }
            }
            assert!(
                !ieee_literal_retained(&source, &[changed]),
                "mutation {mutation}"
            );
        }
    }
}
