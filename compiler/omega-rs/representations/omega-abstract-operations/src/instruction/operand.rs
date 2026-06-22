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
    },
    RuntimeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
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
    ImmediateInteger(i64),
    ByteLength(usize),
}
