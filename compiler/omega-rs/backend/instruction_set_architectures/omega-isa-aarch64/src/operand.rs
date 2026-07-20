#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallOperand {
    DataAddress,
    /// The content pointer of a text argument. For a `{ptr, len}` descriptor
    /// the pointer is LOADED at `byte_offset`; for an owned `[u8; N]` carrier
    /// (`is_bounded_buffer`) it is the COMPUTED inline-bytes address
    /// `base + byte_offset + 8` — same width either way (one add vs one load).
    RuntimeStringPointer {
        byte_offset: usize,
        is_bounded_buffer: bool,
    },
    /// The length of a text argument: at `byte_offset + 8` for a descriptor,
    /// at `byte_offset` (the carrier's leading len word) for an owned carrier.
    RuntimeStringLength {
        byte_offset: usize,
        is_bounded_buffer: bool,
    },
    RuntimePointeeStringPointer {
        byte_offset: usize,
    },
    RuntimePointeeStringLength {
        byte_offset: usize,
    },
    RuntimeScalarInteger {
        byte_offset: usize,
        byte_count: usize,
    },
    /// A floating-point scalar (`f32`/`f64`) loaded into a FLOAT argument register
    /// (v0–v7) via the vector-register arg sequence, independent of the x-register
    /// sequence. The encoder loads the bits into a scratch GPR then `fmov`s into the
    /// next v-register. `byte_count` is 4 or 8. (Cocoa/Core Graphics/libm doubles.)
    RuntimeScalarFloat {
        byte_offset: usize,
        byte_count: usize,
    },
    /// One flat by-value HFA source. The normalized `ValuePlacement` supplies
    /// the exact vector register for every member fragment.
    RuntimeHomogeneousFloatAggregate {
        byte_offset: usize,
        member_byte_count: usize,
        members: u8,
    },
    RuntimeSmallAggregate {
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
    },
    RuntimeLargeAggregate {
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
    },
    /// The ADDRESS of a runtime storage place (a caller buffer/out-param
    /// pointer): `adrp`+`add` to the region base (relocated), then `add` the
    /// field byte offset. Unlike `RuntimeScalarInteger` it does not load the
    /// value — the pointer itself is the argument.
    RuntimeStorageAddress {
        byte_offset: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}
