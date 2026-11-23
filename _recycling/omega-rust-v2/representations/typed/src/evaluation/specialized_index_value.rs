use crate::{StaticValue, StaticValueRequest, ValueType};
/// L4 evaluates the indicated source operation after applying the owner's static arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecializedIndexValue {
    pub request: StaticValueRequest,
    pub arguments: Vec<ValueType>,
    pub value: StaticValue,
}
