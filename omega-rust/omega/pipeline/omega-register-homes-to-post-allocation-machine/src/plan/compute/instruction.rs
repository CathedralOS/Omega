//! Physical operand footprints and complete instruction effects.

use std::collections::BTreeSet;

use omega_register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::SelectedInstruction;

use crate::PostAllocationMachineError;
use omega_physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use omega_selected_instructions::InstructionMachineEffects;

use super::alternative;

pub(super) fn build(
    function_index: usize,
    selected: &SelectedInstruction,
    effects: &InstructionMachineEffects,
    homes: &omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
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
            let home = homes
                .assignments
                .iter()
                .find(|home| home.virtual_register == operand.virtual_register)
                .ok_or(PostAllocationMachineError::MissingHome {
                    function: function_index,
                    register: operand.virtual_register.0,
                })?;
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == home.view)
                .ok_or(PostAllocationMachineError::UnknownView {
                    function: function_index,
                    register: operand.virtual_register.0,
                    view: home.view.0,
                })?;
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
