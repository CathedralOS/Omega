use crate::RuntimeStorageRegion;
use crate::TargetDataObjectHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInstructionOperand {
    pub kind: TargetInstructionOperandKind,
}

impl Default for TargetInstructionOperand {
    fn default() -> Self {
        Self {
            kind: TargetInstructionOperandKind::ImmediateInteger(0),
        }
    }
}

pub trait InstructionOperandLike {
    fn data_address(&self) -> Option<TargetDataObjectHandle>;
    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)>;
    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)>;
    /// Whether a `runtime_string_pointer`/`runtime_string_length` operand reads an
    /// owned `[u8; N]` carrier (`{len, bytes}` inline) rather than a `{ptr, len}`
    /// descriptor -- so the host-call encoder uses carrier addressing (content at
    /// `place + pointer_size`, length at offset 0). `false` for any other operand.
    fn runtime_string_is_bounded_buffer(&self) -> bool;
    fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)>;
    fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)>;
    /// A scalar integer read directly from a runtime-storage slot: `(region, byte_offset,
    /// byte_count)`. Used to marshal a non-constant exit code / host-call argument.
    fn runtime_scalar_integer(&self) -> Option<(RuntimeStorageRegion, usize, usize)>;
    fn immediate_integer(&self) -> Option<i64>;
    fn byte_length(&self) -> Option<usize>;
}

impl InstructionOperandLike for TargetInstructionOperand {
    fn data_address(&self) -> Option<TargetDataObjectHandle> {
        match self.kind {
            InstructionOperandKind::DataAddress { data } => Some(data),
            _ => None,
        }
    }

    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
                ..
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
                ..
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_is_bounded_buffer(&self) -> bool {
        matches!(
            self.kind,
            InstructionOperandKind::RuntimeStringPointer {
                is_bounded_buffer: true,
                ..
            } | InstructionOperandKind::RuntimeStringLength {
                is_bounded_buffer: true,
                ..
            }
        )
    }

    fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimePointeeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimePointeeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_scalar_integer(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeScalarInteger {
                region,
                byte_offset,
                byte_count,
            } => Some((region, byte_offset, byte_count)),
            _ => None,
        }
    }

    fn immediate_integer(&self) -> Option<i64> {
        match self.kind {
            InstructionOperandKind::ImmediateInteger(value) => Some(value),
            _ => None,
        }
    }

    fn byte_length(&self) -> Option<usize> {
        match self.kind {
            InstructionOperandKind::ByteLength(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetInstructionOperandKind {
    DataAddress {
        data: TargetDataObjectHandle,
    },
    RuntimeStringPointer {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        /// The place is an owned `[u8; N]` carrier (`{len, bytes}` inline): the
        /// content pointer is the COMPUTED address `place + pointer_size` (the
        /// inline bytes), not a stored descriptor pointer at offset 0.
        is_bounded_buffer: bool,
    },
    RuntimeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        /// Owned carrier: the length is read at offset 0, not the descriptor's
        /// length word at offset `pointer_size`.
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
    RuntimeScalarInteger {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}

pub type InstructionOperand = TargetInstructionOperand;
pub type InstructionOperandKind = TargetInstructionOperandKind;
