#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    RuntimeStringPointer { byte_offset: usize },
    RuntimeStringLength { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
