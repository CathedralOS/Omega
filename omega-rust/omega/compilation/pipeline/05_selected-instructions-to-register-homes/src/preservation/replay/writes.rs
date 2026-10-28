use std::collections::BTreeMap;

use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{SelectedBlockId, SelectedInstruction, VirtualRegisterId};

use super::{
    super::{AllocatedCalleeSavedRequirementError, CalleeSavedModificationWitness},
    state::{ReplayTraversal, add},
};

pub(super) fn scan_instruction(
    traversal: &mut ReplayTraversal<'_>,
    machine: semantic_vocabulary::MachineId,
    block: SelectedBlockId,
    instruction: &SelectedInstruction,
    homes: &BTreeMap<VirtualRegisterId, &crate::VirtualRegisterHome>,
    units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    traversal.instruction_count = add(traversal.instruction_count, 1)?;
    for operand in &instruction.operands {
        traversal.operand_count = add(traversal.operand_count, 1)?;
        if !matches!(
            operand.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        ) {
            continue;
        }
        let home = homes.get(&operand.virtual_register).ok_or(
            AllocatedCalleeSavedRequirementError::MissingHome {
                function: machine,
                virtual_register: operand.virtual_register,
            },
        )?;
        let view = traversal
            .physical
            .model()
            .views
            .iter()
            .find(|view| {
                home.class == operand.class
                    && operand.fixed_view.is_none_or(|fixed| fixed == home.view)
                    && view.id == home.view
                    && view.class == operand.class
            })
            .ok_or(
                AllocatedCalleeSavedRequirementError::UnknownOrIncompatibleView {
                    function: machine,
                    virtual_register: operand.virtual_register,
                    view: home.view,
                },
            )?;
        traversal.scan_units(
            &view.write_units,
            CalleeSavedModificationWitness::OperandDefinition {
                block,
                instruction: instruction.id,
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                home_view: home.view,
                write_semantics: view.write_semantics,
            },
            units,
        )?;
    }
    traversal.scan_implicit(
        block,
        instruction.id,
        &instruction.implicit_defs,
        &instruction.clobbers,
        units,
    )
}

pub(super) fn index_homes(
    homes: &crate::FunctionRegisterHomes,
) -> Result<
    BTreeMap<VirtualRegisterId, &crate::VirtualRegisterHome>,
    AllocatedCalleeSavedRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for home in &homes.assignments {
        if keyed.insert(home.virtual_register, home).is_some() {
            return Err(AllocatedCalleeSavedRequirementError::DuplicateHome {
                function: homes.machine,
                virtual_register: home.virtual_register,
            });
        }
    }
    Ok(keyed)
}
