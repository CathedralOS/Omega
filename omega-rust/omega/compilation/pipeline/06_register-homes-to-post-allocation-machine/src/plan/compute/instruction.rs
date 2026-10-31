//! Physical operand footprints and complete instruction effects.

use std::collections::BTreeSet;

use register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use selected_instructions::SelectedInstruction;
use selected_instructions::SelectedInstructionKind as Kind;

use crate::PostAllocationMachineError;
use physical_instructions::PhysicalAddressOperation as Address;
use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use selected_instructions::InstructionMachineEffects;

use super::alternative;
use super::homes::PreparedHomes;

pub(super) fn build(
    function_index: usize,
    selected: &SelectedInstruction,
    effects: &InstructionMachineEffects,
    homes: &PreparedHomes<'_>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineError> {
    if effects.instruction != selected.id || effects.kind != selected.kind {
        return Err(PostAllocationMachineError::InstructionMismatch {
            function: function_index,
            instruction: selected.id.0,
        });
    }
    let operands = selected
        .operands
        .iter()
        .map(|operand| {
            let home = homes.get(operand.virtual_register).ok_or(
                PostAllocationMachineError::MissingHome {
                    function: function_index,
                    register: operand.virtual_register.0,
                },
            )?;
            let view = physical.model().view(home.view).ok_or(
                PostAllocationMachineError::UnknownView {
                    function: function_index,
                    register: operand.virtual_register.0,
                    view: home.view.0,
                },
            )?;
            if home.class != operand.class || view.class != operand.class {
                return Err(PostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            if operand.fixed_view.is_some_and(|fixed| fixed != home.view) {
                return Err(PostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            let reads = reads(operand.access);
            let writes = writes(operand.access);
            Ok(PhysicalOperandFootprint {
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                class: operand.class,
                view: home.view,
                access: operand.access,
                storage_units: view.units.clone(),
                read_units: if reads {
                    view.units.clone()
                } else {
                    Vec::new()
                },
                write_units: if writes {
                    view.write_units.clone()
                } else {
                    Vec::new()
                },
                write_semantics: writes.then_some(view.write_semantics),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let alternative =
        alternative::choose(selected.id.0, &operands, &effects.alternatives, physical)?;
    let mut unit_uses = effects.unit_uses.iter().copied().collect::<BTreeSet<_>>();
    let mut unit_defs = effects.unit_defs.iter().copied().collect::<BTreeSet<_>>();
    for operand in &operands {
        unit_uses.extend(&operand.read_units);
        unit_defs.extend(&operand.write_units);
    }
    Ok(PostAllocationMachineInstruction {
        instruction: selected.id,
        alternative,
        operands,
        address: match selected.kind {
            Kind::Load64 { byte_offset } => Some(Address::Load64 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load8 { byte_offset } => Some(Address::Load8 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load16 { byte_offset } => Some(Address::Load16 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load32 { byte_offset } => Some(Address::Load32 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::LoadPacked { byte_offset, width } => Some(Address::LoadPacked {
                base_operand: 0,
                byte_offset,
                width,
            }),
            Kind::StorePacked { byte_offset, width } => Some(Address::StorePacked {
                base_operand: 0,
                byte_offset,
                width,
            }),
            Kind::Store {
                byte_offset,
                byte_size,
            } => Some(Address::Store {
                base_operand: 0,
                byte_offset,
                byte_size,
            }),
            Kind::AddressOffset { byte_offset } => Some(Address::AddressOffset {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Store64 { slot, byte_offset } => Some(Address::Store64 { slot, byte_offset }),
            Kind::FrameAddress { slot, byte_offset } => {
                Some(Address::FrameAddress { slot, byte_offset })
            }
            Kind::Load8Indexed => Some(Address::Load8Indexed {
                base_operand: 0,
                index_operand: 1,
            }),
            Kind::AccessorScalarAccess {
                form,
                contract,
                byte_offset,
            } => Some(Address::AccessorScalarAccess {
                base_operand: 0,
                byte_offset,
                form,
                contract,
            }),
            Kind::HostedReadByte { slot } => Some(Address::HostedReadByte { slot }),
            Kind::HostedWriteByteI32 { slot } => Some(Address::HostedWriteByteI32 { slot }),
            Kind::SaveFloatingControl { slot } => Some(Address::SaveFloatingControl { slot }),
            Kind::RestoreFloatingControl { slot } => Some(Address::RestoreFloatingControl { slot }),
            _ => None,
        },
        implicit_unit_uses: effects.unit_uses.clone(),
        implicit_unit_defs: effects.unit_defs.clone(),
        implicit_unit_clobbers: effects.unit_clobbers.clone(),
        unit_uses: unit_uses.into_iter().collect(),
        unit_defs: unit_defs.into_iter().collect(),
        unit_clobbers: effects.unit_clobbers.clone(),
    })
}

const fn reads(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

const fn writes(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}
