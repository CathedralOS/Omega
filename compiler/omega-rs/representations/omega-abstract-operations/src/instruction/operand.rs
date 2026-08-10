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
    /// A floating-point scalar (`f32`/`f64`) read from a runtime-storage slot and
    /// marshalled into a FLOAT argument register (arm64 v0–v7), NOT a GPR. arm64
    /// passes float/double args in the vector-register sequence, independent of the
    /// x-register sequence used for integers/pointers. `byte_count` is 4 (`f32`) or
    /// 8 (`f64`). Used by Cocoa, Core Graphics, libm, and other native calls taking
    /// a `CGFloat`/`double`.
    RuntimeScalarFloat {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
    },
    /// A flat native homogeneous floating-point aggregate read from one
    /// runtime-storage place. AAPCS64 spreads members across vector registers;
    /// SysV AMD64 packs them by eightbyte. Preserving the aggregate here
    /// prevents selection from degrading a by-value record into a pointer.
    RuntimeHomogeneousFloatAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        member_byte_count: usize,
        members: u8,
    },
    /// A two-eightbyte SysV AMD64 aggregate with at least one SSE-class
    /// eightbyte. Bit 0/1 of `sse_eightbytes` identifies the corresponding
    /// SSE-class eightbyte; all-INTEGER records use `RuntimeSmallAggregate`.
    RuntimeSystemVAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
        sse_eightbytes: u8,
    },
    /// A fixed non-HFA native aggregate of 9-16 bytes, preserved by value so
    /// lowering can copy its doubleword fragments into registers or the stack.
    RuntimeSmallAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
    },
    /// A fixed non-HFA native aggregate above 16 bytes. AAPCS64 passes a
    /// caller copy indirectly; SysV AMD64 places the value in outgoing stack
    /// storage.
    RuntimeLargeAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
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
