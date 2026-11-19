use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedInstruction {
    pub site: Site,
    pub target: String,
    pub mnemonic: String,
    pub operands: Vec<ValueType>,
}
