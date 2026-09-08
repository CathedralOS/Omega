use super::*;
use semantic_vocabulary::{IeeeFloatValue, OperationId, ValueId};
use target_operations::TargetUnitOperation;

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
