use std::collections::BTreeMap;

use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{SelectedBlockId, SelectedInstruction, VirtualRegisterId};

use super::{
    super::{AllocatedCalleeSavedRequirementError, CalleeSavedModificationWitness},
    state::{ReplayTraversal, add},
};

pub(super) fn scan_instruction(
    traversal: &mut ReplayTraversal<'_>,
    machine: psi_core::MachineId,
    block: SelectedBlockId,
    instruction: &SelectedInstruction,
    homes: &BTreeMap<
        VirtualRegisterId,
        &omega_selected_instructions_to_register_homes::VirtualRegisterHome,
    >,
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
        scan_units(
            traversal,
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
    scan_implicit(
        traversal,
        block,
        instruction.id,
        &instruction.implicit_defs,
        &instruction.clobbers,
        units,
    )
}

pub(super) fn scan_implicit(
    traversal: &mut ReplayTraversal<'_>,
    block: SelectedBlockId,
    instruction: omega_selected_instructions::SelectedInstructionId,
    definitions: &[RegisterUnitId],
    clobbers: &[RegisterUnitId],
    units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    scan_units(
        traversal,
        definitions,
        CalleeSavedModificationWitness::ImplicitDefinition { block, instruction },
        units,
    )?;
    scan_units(
        traversal,
        clobbers,
        CalleeSavedModificationWitness::ImplicitClobber { block, instruction },
        units,
    )
}

fn scan_units(
    traversal: &mut ReplayTraversal<'_>,
    writes: &[RegisterUnitId],
    witness: CalleeSavedModificationWitness,
    units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    for unit in writes {
        traversal.write_count = add(traversal.write_count, 1)?;
        if traversal.callee_saved.contains(unit) {
            traversal.witness_count = add(traversal.witness_count, 1)?;
            units.entry(*unit).or_default().push(witness);
        }
    }
    Ok(())
}

pub(super) fn index_homes(
    homes: &omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
) -> Result<
    BTreeMap<
        VirtualRegisterId,
        &omega_selected_instructions_to_register_homes::VirtualRegisterHome,
    >,
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
