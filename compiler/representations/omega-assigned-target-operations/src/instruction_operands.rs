use omega_target_operations::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedInstructionOperand {
    pub kind: AssignedInstructionOperandKind,
}

impl Default for AssignedInstructionOperand {
    fn default() -> Self {
        Self {
            kind: AssignedInstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedInstructionOperandKind {
    DataAddress {
        data: omega_target_operations::TargetDataObjectHandle,
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
    ImmediateInteger(i64),
    ByteLength(usize),
}

impl From<omega_target_operations::TargetInstructionOperandKind>
    for AssignedInstructionOperandKind
{
    fn from(kind: omega_target_operations::TargetInstructionOperandKind) -> Self {
        match kind {
            omega_target_operations::TargetInstructionOperandKind::DataAddress { data } => {
                Self::DataAddress { data }
            }
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::RuntimePointeeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimePointeeStringPointer {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::RuntimePointeeStringLength {
                region,
                byte_offset,
            } => Self::RuntimePointeeStringLength {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            omega_target_operations::TargetInstructionOperandKind::ByteLength(value) => {
                Self::ByteLength(value)
            }
        }
    }
}

impl From<AssignedInstructionOperandKind>
    for omega_target_operations::TargetInstructionOperandKind
{
    fn from(kind: AssignedInstructionOperandKind) -> Self {
        match kind {
            AssignedInstructionOperandKind::DataAddress { data } => Self::DataAddress { data },
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::RuntimePointeeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimePointeeStringPointer {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::RuntimePointeeStringLength {
                region,
                byte_offset,
            } => Self::RuntimePointeeStringLength {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            AssignedInstructionOperandKind::ByteLength(value) => Self::ByteLength(value),
        }
    }
}

pub type InstructionOperand = AssignedInstructionOperand;
pub type InstructionOperandKind = AssignedInstructionOperandKind;

impl omega_target_operations::InstructionOperandLike for AssignedInstructionOperand {
    fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> {
        match self.kind {
            AssignedInstructionOperandKind::DataAddress { data } => Some(data),
            _ => None,
        }
    }

    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimePointeeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimePointeeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn immediate_integer(&self) -> Option<i64> {
        match self.kind {
            AssignedInstructionOperandKind::ImmediateInteger(value) => Some(value),
            _ => None,
        }
    }

    fn byte_length(&self) -> Option<usize> {
        match self.kind {
            AssignedInstructionOperandKind::ByteLength(value) => Some(value),
            _ => None,
        }
    }
}
