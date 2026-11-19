use crate::{Site, ValueType};
use num_bigint::BigInt;
/// L4 evaluates the indicated source operation after applying the owner's static arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecializedIndexValue {
    pub expression: Site,
    pub arguments: Vec<ValueType>,
    pub value: BigInt,
}
