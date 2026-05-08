#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    RuntimeMachineStringPointer { byte_offset: usize },
    RuntimeMachineStringLength { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
