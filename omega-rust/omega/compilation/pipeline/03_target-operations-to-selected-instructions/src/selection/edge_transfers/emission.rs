//! One word-emission mechanism for edge transfers.
//!
//! Each edge-transfer transport stages 64-bit address words: a frame address,
//! an address displacement, a loaded extent word, or the word it stores into
//! its own join slot. Descriptor, scalar-reference and address transports each
//! restated that emission — the selected-CFG bundle it writes, identity
//! allocation, the ABI-transport register, its class, its physical footprint —
//! and the copies drifted: descriptor transports took the class from the source
//! register while the other two took the class the emitted row declares, which
//! is the rule the register-home assignment checks. One context and one emitter
//! hold the shared policy; a transport supplies only the instruction it needs.
use super::VirtualRegisterId;
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::invalid;
use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedSelectionConstraints, VirtualRegister, VirtualRegisterOrigin,
};
use semantic_vocabulary::{EdgeId, PlaceId};

/// The selected-CFG bundle one edge's transports write, with the target
/// environment and constraint catalog they emit against.
pub(super) struct Emission<'state> {
    pub(super) function: usize,
    pub(super) registers: &'state mut Vec<VirtualRegister>,
    pub(super) memory: &'state mut Vec<SelectedMemoryAccess>,
    pub(super) instructions: &'state mut Vec<SelectedInstruction>,
    pub(super) next_instruction: &'state mut usize,
    pub(super) edge: EdgeId,
    pub(super) environment: &'state register_environment::ValidatedTargetRegisterEnvironment,
    pub(super) constraints: &'state SelectedSelectionConstraints,
    pub(super) catalog: &'state ValidatedRegisterConstraintCatalog,
}

impl Emission<'_> {
    /// The edge's next instruction identity. Preparation reserves every
    /// detached cleanup row before this counter starts.
    pub(super) fn instruction_id(
        &mut self,
    ) -> Result<SelectedInstructionId, SelectedInstructionError> {
        let function = self.function;
        let id = SelectedInstructionId(
            (*self.next_instruction)
                .try_into()
                .map_err(|_| invalid(function))?,
        );
        *self.next_instruction += 1;
        Ok(id)
    }

    /// Emit one instruction whose single output is a fresh transfer word. The
    /// class is the one the emitted row declares for that operand — the class
    /// the register-home assignment requires of the register's definition.
    /// `access` records the physical footprint on the same identity.
    pub(super) fn emit(
        &mut self,
        kind: SelectedInstructionKind,
        key: register_model::RegisterConstraintKey,
        inputs: &[VirtualRegisterId],
        place: PlaceId,
        byte_offset: u32,
        access: Option<(u32, SelectedMemoryAccessRole)>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let function = self.function;
        let instruction_id = self.instruction_id()?;
        let output = VirtualRegisterId(
            self.registers
                .len()
                .try_into()
                .map_err(|_| invalid(function))?,
        );
        let mut operands = inputs.to_vec();
        operands.push(output);
        let instruction = super::super::constraints::instruction(
            instruction_id,
            kind,
            key,
            &operands,
            provenance(self.edge),
            self.catalog,
        )?;
        let class = instruction
            .operands
            .last()
            .ok_or_else(|| invalid(function))?
            .class;
        self.registers.push(VirtualRegister {
            id: output,
            scalar_type: pointer_type(function)?,
            class,
            origin: VirtualRegisterOrigin::AbiTransport {
                instruction: instruction_id,
                place,
                byte_offset,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        if let Some((byte_count, role)) = access {
            self.memory.push(SelectedMemoryAccess {
                instruction: instruction_id,
                origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                place,
                byte_offset,
                byte_count,
                role,
            });
        }
        self.instructions.push(instruction);
        Ok(output)
    }

    /// Store one staged word into an edge local's own slot, recording the
    /// write footprint on the same identity.
    pub(super) fn store_local_word(
        &mut self,
        slot: LocalStorageSlotId,
        byte_offset: u32,
        value: VirtualRegisterId,
        place: PlaceId,
    ) -> Result<(), SelectedInstructionError> {
        let function = self.function;
        let instruction_id = self.instruction_id()?;
        let key = self
            .constraints
            .keys
            .store64
            .ok_or_else(|| invalid(function))?;
        let row = super::super::constraints::row(self.catalog, key)?;
        let operands = super::super::constraints::store_registers(
            instruction_id,
            &[value],
            row,
            &mut *self.registers,
        )?;
        self.instructions
            .push(super::super::constraints::instruction(
                instruction_id,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset,
                },
                key,
                &operands,
                provenance(self.edge),
                self.catalog,
            )?);
        self.memory.push(SelectedMemoryAccess {
            instruction: instruction_id,
            origin: SelectedMemoryAccessOrigin::Edge(self.edge),
            place,
            byte_offset,
            byte_count: 8,
            role: SelectedMemoryAccessRole::WriteLocal { slot },
        });
        Ok(())
    }

    /// The read footprint one binding's source extent realizes at the access
    /// just emitted.
    pub(super) fn source_reads(
        &mut self,
        binding: &selected_instructions::SelectedStructuralBinding,
        byte_offset: u32,
    ) -> Result<(), SelectedInstructionError> {
        let function = self.function;
        let instruction = self
            .instructions
            .last()
            .ok_or_else(|| invalid(function))?
            .id;
        self.memory.extend(
            binding
                .source_read_roles()
                .map(|role| SelectedMemoryAccess {
                    instruction,
                    origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                    place: binding.semantic.argument.place,
                    byte_offset,
                    byte_count: 8,
                    role,
                }),
        );
        Ok(())
    }
}

/// Every transfer word is a 64-bit pointer or address word.
pub(super) fn pointer_type(
    function: usize,
) -> Result<semantic_vocabulary::ScalarType, SelectedInstructionError> {
    semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
        .map(semantic_vocabulary::ScalarType::Integer)
        .map_err(|_| invalid(function))
}

pub(super) fn provenance(edge: EdgeId) -> selected_instructions::SelectedInstructionProvenance {
    selected_instructions::SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    }
}
