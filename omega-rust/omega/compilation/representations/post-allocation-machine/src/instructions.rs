// We define `PostAllocationMachineInstruction`, the `MachineAlternativeChoiceRule`
// legality rule, and symbolic `PhysicalAddressOperation` memory/frame addressing
// payloads resolved once a concrete frame layout is supplied.

use crate::PhysicalOperandFootprint;
use register_model::RegisterUnitId;
use selected_instructions::{MachineAlternative, SelectedInstructionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineAlternativeChoiceRule {
    UniqueApplicableInCatalogOrderV1, // Target catalogs partition physical-home configurations so exactly one alternative applies.
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
    pub fn operand_footprint(&self, operand: u16) -> Option<&PhysicalOperandFootprint> {
        self.operands.iter().find(|row| row.operand == operand)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalAddressOperation {
    AccessorScalarAccess {
        base_operand: u16,
        byte_offset: u32,
        form: selected_instructions::AccessorScalarForm,
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
