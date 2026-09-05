//! Canonical construction of one derived value-range fact.

use optimization_unit::{
    ValueRangeFact, ValueRangeRegion, ValueRangeSupport, value_range_fact_identity,
};
use semantic_vocabulary::{IntegerType, IntegerValue, ValueId};

pub(super) fn new(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: ValueRangeSupport,
    valid_in: ValueRangeRegion,
) -> ValueRangeFact {
    let identity =
        value_range_fact_identity(value, scalar_type, minimum, maximum, &support, &valid_in)
            .expect("internally derived value range has a canonical identity");
    ValueRangeFact {
        identity,
        value,
        scalar_type,
        minimum,
        maximum,
        support,
        valid_in,
    }
}
