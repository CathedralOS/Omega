#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    RuntimeStringPointer { byte_offset: usize },
    RuntimeStringLength { byte_offset: usize },
    RuntimePointeeStringPointer { byte_offset: usize },
    RuntimePointeeStringLength { byte_offset: usize },
    RuntimeScalarInteger { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
