//! Revision-bound integer ranges backed by independently reconstructable scalar constants.

use super::{
    IntegerCarrier, IntegerType, IntegerValue, ScalarConstantFactIdentity, ValueId,
    ValueRangeFactIdentity,
};
use super::{MachineId, OptimizationUnitIdentity};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeSupport {
    ScalarConstant(ScalarConstantFactIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeRegion {
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub value: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeFact {
    pub identity: ValueRangeFactIdentity,
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
    pub support: ValueRangeSupport,
    pub valid_in: ValueRangeRegion,
}

pub fn value_range_fact_identity(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: &ValueRangeSupport,
    valid_in: &ValueRangeRegion,
) -> Option<ValueRangeFactIdentity> {
    if valid_in.value != value
        || scalar_type.carrier() != IntegerCarrier::Fixed
        || !scalar_type.admits(minimum)
        || !scalar_type.admits(maximum)
        || integer_value_cmp(scalar_type, minimum, maximum).is_none_or(|order| order.is_gt())
    {
        return None;
    }
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-value-range-fact.v1\0");
    canonical.extend_from_slice(&valid_in.revision.bytes());
    crate::codec_bytes::push_encoded(&mut canonical, valid_in.machine.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, valid_in.value.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, value.get().to_le_bytes());
    encode_integer_type(&mut canonical, scalar_type);
    encode_integer_value(&mut canonical, minimum);
    encode_integer_value(&mut canonical, maximum);
    let ValueRangeSupport::ScalarConstant(identity) = support;
    canonical.push(1);
    canonical.extend_from_slice(&identity.bytes());
    canonical.push(1); // Entire-value scope.
    canonical.extend_from_slice(&0_u64.to_le_bytes()); // No dominated-block restriction.
    Some(ValueRangeFactIdentity::from_canonical_bytes(&canonical))
}

use semantic_vocabulary::IntegerSign;

pub(crate) fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

pub(crate) fn encode_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    crate::codec_bytes::push_encoded(canonical, scalar_type.bits().to_le_bytes());
}

pub(crate) fn encode_integer_value(canonical: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            canonical.push(1);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            canonical.push(2);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
    }
}
