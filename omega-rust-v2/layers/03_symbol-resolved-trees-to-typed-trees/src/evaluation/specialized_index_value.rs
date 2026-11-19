use crate::{StaticIntegerRequest, ValueType};
use num_bigint::BigInt;
/// L4 evaluates the indicated source operation after applying the owner's static arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecializedIndexValue {
    pub request: StaticIntegerRequest,
    pub arguments: Vec<ValueType>,
    pub value: BigInt,
}
