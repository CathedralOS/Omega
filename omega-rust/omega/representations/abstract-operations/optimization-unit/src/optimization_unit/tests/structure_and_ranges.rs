use super::fixtures::id;
use crate::{
    OptimizationUnitIdentity, ScalarConstantFactIdentity, ValueRangeRegion, ValueRangeScope,
    ValueRangeSupport, value_range_fact_identity,
};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ValueId,
};

#[test]
fn value_range_identity_rejects_malformed_type_support_and_region_axes() {
    let revision = OptimizationUnitIdentity::from_canonical_bytes(b"range revision");
    let machine = id(91, MachineId::new);
    let block = id(92, BlockId::new);
    let value = id(93, ValueId::new);
    let other_value = id(94, ValueId::new);
    let operation = id(95, OperationId::new);
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let support = ValueRangeSupport::ScalarConstant(
        ScalarConstantFactIdentity::from_canonical_bytes(b"range constant"),
    );
    let entire = ValueRangeRegion {
        revision,
        machine,
        value,
        scope: ValueRangeScope::EntireValue,
        dominated_blocks: Vec::new(),
    };
    let baseline = value_range_fact_identity(
        value,
        scalar_type,
        IntegerValue::Unsigned(1),
        IntegerValue::Unsigned(7),
        &support,
        &entire,
    )
    .expect("well-formed range identity");
    assert_ne!(
        baseline,
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(8),
            &support,
            &entire,
        )
        .unwrap()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                value: other_value,
                ..entire.clone()
            },
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(8),
            IntegerValue::Unsigned(7),
            &support,
            &entire,
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                scope: ValueRangeScope::DominatedOperationEntry {
                    block,
                    node: 1,
                    operation,
                },
                dominated_blocks: vec![block],
                ..entire.clone()
            },
        )
        .is_none()
    );
}
