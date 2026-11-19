use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedInstruction {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub target: String,
    pub mnemonic: String,
    pub operands: Vec<ValueType>,
}
