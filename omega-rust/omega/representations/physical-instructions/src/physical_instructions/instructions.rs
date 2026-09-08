//! Selected machine alternatives and complete physical register actions.

use crate::PhysicalOperandFootprint;
use register_model::RegisterUnitId;
use selected_instructions::{MachineAlternative, SelectedInstructionId};

/// This is a legality rule, not an optimization level or cost policy. Current
/// target catalogs must partition physical-home configurations so exactly one
/// declared alternative applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineAlternativeChoiceRule {
    UniqueApplicableInCatalogOrderV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineInstruction {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternative,
    pub operands: Vec<PhysicalOperandFootprint>,
    pub address: Option<PhysicalAddressOperation>,
    pub implicit_unit_uses: Vec<RegisterUnitId>,
    pub implicit_unit_defs: Vec<RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<RegisterUnitId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
}

/// Address semantics remain symbolic until a validated frame is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalAddressOperation {
    HostedReadByte {
        slot: selected_instructions::LocalStorageSlotId,
    },
    HostedWriteByteI32 {
        slot: selected_instructions::LocalStorageSlotId,
    },
    Store {
        base_operand: u16,
        byte_offset: u32,
        byte_size: u8,
    },
    AddressOffset {
        base_operand: u16,
        byte_offset: u32,
    },
    Load8Indexed {
        base_operand: u16,
        index_operand: u16,
    },
    Load64 {
        base_operand: u16,
        byte_offset: u32,
    },
    Load32 {
        base_operand: u16,
        byte_offset: u32,
    },
    Store64 {
        slot: selected_instructions::FrameStorageSlotId,
        byte_offset: u32,
    },
    FrameAddress {
        slot: selected_instructions::FrameStorageSlotId,
        byte_offset: u32,
    },
}
