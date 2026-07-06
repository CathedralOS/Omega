#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    RuntimeStringPointer { byte_offset: usize },
    RuntimeStringLength { byte_offset: usize },
    RuntimePointeeStringPointer { byte_offset: usize },
    RuntimePointeeStringLength { byte_offset: usize },
    RuntimeScalarInteger { byte_offset: usize, byte_count: usize },
    /// The ADDRESS of a runtime storage place (a caller buffer/out-param
    /// pointer): `adrp`+`add` to the region base (relocated), then `add` the
    /// field byte offset. Unlike `RuntimeScalarInteger` it does not load the
    /// value — the pointer itself is the argument.
    RuntimeStorageAddress { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
