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
    /// A floating-point scalar read from a runtime-storage slot: `(region,
    /// byte_offset, byte_count)`. Marshalled into a float argument register (v0–v7).
    fn runtime_scalar_float(&self) -> Option<(RuntimeStorageRegion, usize, usize)>;
    /// A flat homogeneous floating-point aggregate: region, byte offset,
    /// equal member width, and member count.
    fn runtime_homogeneous_float_aggregate(
        &self,
    ) -> Option<(RuntimeStorageRegion, usize, usize, u8)> {
        None
    }
    fn runtime_system_v_aggregate(
        &self,
    ) -> Option<(RuntimeStorageRegion, usize, usize, usize, u8)> {
        None
    }
    fn runtime_small_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
        None
    }
    fn runtime_large_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
        None
    }
    /// The ADDRESS of a runtime-storage place, `(region, byte_offset)`, marshalled
    /// as a pointer-sized host-call argument (`lea` through the relocated region
    /// base) -- the extern boundary's pointer-argument shape.
    fn runtime_storage_address(&self) -> Option<(RuntimeStorageRegion, usize)>;
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

    fn runtime_scalar_float(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeScalarFloat {
                region,
                byte_offset,
                byte_count,
            } => Some((region, byte_offset, byte_count)),
            _ => None,
        }
    }

    fn runtime_homogeneous_float_aggregate(
        &self,
    ) -> Option<(RuntimeStorageRegion, usize, usize, u8)> {
        match self.kind {
            InstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                region,
                byte_offset,
                member_byte_count,
                members,
            } => Some((region, byte_offset, member_byte_count, members)),
            _ => None,
        }
    }

    fn runtime_system_v_aggregate(
        &self,
    ) -> Option<(RuntimeStorageRegion, usize, usize, usize, u8)> {
        match self.kind {
            InstructionOperandKind::RuntimeSystemVAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
                sse_eightbytes,
            } => Some((region, byte_offset, byte_count, alignment, sse_eightbytes)),
            _ => None,
        }
    }

    fn runtime_small_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeSmallAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
            } => Some((region, byte_offset, byte_count, alignment)),
            _ => None,
        }
    }

    fn runtime_large_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeLargeAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
            } => Some((region, byte_offset, byte_count, alignment)),
            _ => None,
        }
    }

    fn runtime_storage_address(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeStorageAddress {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
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
    /// A floating-point scalar (`f32`/`f64`) marshalled into a FLOAT argument
    /// register (arm64 v0–v7), independent of the integer/pointer x-register
    /// sequence. `byte_count` is 4 or 8. (Cocoa/Core Graphics/libm `double` args.)
    RuntimeScalarFloat {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
    },
    RuntimeHomogeneousFloatAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        member_byte_count: usize,
        members: u8,
    },
    RuntimeSystemVAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
        sse_eightbytes: u8,
    },
    RuntimeSmallAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
    },
    RuntimeLargeAggregate {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_count: usize,
        alignment: usize,
    },
    /// The ADDRESS of a statically allocated runtime-storage place (`region` base
    /// + `byte_offset`), marshalled as a pointer-sized host-call argument (the
    /// extern boundary's pointer-argument shape). Encoders emit `lea` through the
    /// relocated region base rather than loading the place's bytes.
    RuntimeStorageAddress {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}

pub type InstructionOperand = TargetInstructionOperand;
pub type InstructionOperandKind = TargetInstructionOperandKind;
