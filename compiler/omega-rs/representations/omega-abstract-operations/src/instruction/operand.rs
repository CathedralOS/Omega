use crate::AbstractDataObjectHandle;
use crate::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionOperand {
    pub kind: InstructionOperandKind,
}

impl Default for InstructionOperand {
    fn default() -> Self {
        Self {
            kind: InstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionOperandKind {
    DataAddress {
        data: AbstractDataObjectHandle,
    },
    RuntimeStringPointer {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        /// The place is an owned `[u8; N]` carrier (`{len, bytes}` inline): the
        /// content pointer is the computed address `place + pointer_size`, not a
        /// stored descriptor pointer at offset 0.
        is_bounded_buffer: bool,
    },
    RuntimeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        /// Owned carrier: length read at offset 0, not offset `pointer_size`.
        is_bounded_buffer: bool,
    },
    RuntimePointeeStringPointer {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    RuntimePointeeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    /// A scalar integer (e.g. an `i32` exit code) read directly from a statically
    /// allocated runtime-storage slot at `byte_offset` in `region`, rather than a
    /// compile-time constant. `byte_count` is the value's width (1/2/4/8).
    RuntimeScalarInteger {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
    },
    /// The ADDRESS of a statically allocated runtime-storage place (`region` base
    /// + `byte_offset`), marshalled as a pointer-sized host-call argument. This is
    /// the pointer-argument shape of the extern boundary: a `[u32; N]` framebuffer
    /// as an `LPVOID`, an OS struct built in an inline array, a NUL-terminated
    /// byte-array C string. The encoders emit `lea` through the relocated region
    /// base rather than loading the place's bytes.
    RuntimeStorageAddress {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}
