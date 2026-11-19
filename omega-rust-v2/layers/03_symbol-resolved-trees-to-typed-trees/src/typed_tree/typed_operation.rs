use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedOperation {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub token: String,
    pub operands: Vec<ValueType>,
    pub result: ValueType,
}
