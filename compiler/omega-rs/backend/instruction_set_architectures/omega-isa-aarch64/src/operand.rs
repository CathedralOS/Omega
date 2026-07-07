#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    RuntimeStringPointer { byte_offset: usize },
    RuntimeStringLength { byte_offset: usize },
    RuntimePointeeStringPointer { byte_offset: usize },
    RuntimePointeeStringLength { byte_offset: usize },
    RuntimeScalarInteger { byte_offset: usize, byte_count: usize },
    /// A floating-point scalar (`f32`/`f64`) loaded into a FLOAT argument register
    /// (v0–v7) via the vector-register arg sequence, independent of the x-register
    /// sequence. The encoder loads the bits into a scratch GPR then `fmov`s into the
    /// next v-register. `byte_count` is 4 or 8. (Cocoa/Core Graphics/libm doubles.)
    RuntimeScalarFloat { byte_offset: usize, byte_count: usize },
    /// The ADDRESS of a runtime storage place (a caller buffer/out-param
    /// pointer): `adrp`+`add` to the region base (relocated), then `add` the
    /// field byte offset. Unlike `RuntimeScalarInteger` it does not load the
    /// value — the pointer itself is the argument.
    RuntimeStorageAddress { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
