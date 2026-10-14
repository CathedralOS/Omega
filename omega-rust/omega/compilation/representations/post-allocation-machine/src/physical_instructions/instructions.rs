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

impl PostAllocationMachineInstruction {
    /// The physical footprint of `operand`, when declared.
    pub fn operand_footprint(&self, operand: u16) -> Option<&PhysicalOperandFootprint> {
        self.operands.iter().find(|row| row.operand == operand)
    }
}

/// Address semantics remain symbolic until a validated frame is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalAddressOperation {
    PlacedScalarAccess {
        base_operand: u16,
        byte_offset: u32,
        form: selected_instructions::PlacedScalarForm,
        contract: [u8; 32],
    },
    SaveFloatingControl {
        slot: selected_instructions::LocalStorageSlotId,
    },
    RestoreFloatingControl {
        slot: selected_instructions::LocalStorageSlotId,
    },
    LoadPacked {
        base_operand: u16,
        byte_offset: u32,
        width: selected_instructions::PackedByteWidth,
    },
    StorePacked {
        base_operand: u16,
        byte_offset: u32,
        width: selected_instructions::PackedByteWidth,
    },
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
    Load8 {
        base_operand: u16,
        byte_offset: u32,
    },
    Load16 {
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
