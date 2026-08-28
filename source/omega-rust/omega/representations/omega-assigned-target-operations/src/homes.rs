use crate::AssignedValueOperandHandle;
use omega_target_operations::RuntimeStorageRegion;

pub type AssignedValueHomeHandle = AssignedValueOperandHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignedRegisterBank {
    #[default]
    GeneralPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64AssignedRegister {
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedRegisterName {
    Aarch64X(u8),
    X86_64(X86_64AssignedRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedValueHomeKind {
    Immediate,
    StackSlot {
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimeStorage {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimePointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameIndexed {
        descriptor_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameFixedIndexed {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    ScratchRegister {
        bank: AssignedRegisterBank,
        name: AssignedRegisterName,
    },
}

impl Default for AssignedValueHomeKind {
    fn default() -> Self {
        Self::Immediate
    }
}
