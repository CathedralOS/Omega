use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{SelectedBlockId, SelectedInstruction};

use super::super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
};

pub(super) struct DirectTraversal<'model> {
    physical: &'model ValidatedPhysicalRegisterModel,
    callee_saved: &'model BTreeSet<RegisterUnitId>,
    pub(super) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
    pub(super) function_count: u64,
    pub(super) block_count: u64,
    pub(super) instruction_count: u64,
    pub(super) operand_count: u64,
    pub(super) write_count: u64,
    pub(super) witness_count: u64,
}

impl<'model> DirectTraversal<'model> {
    pub(super) fn new(
        physical: &'model ValidatedPhysicalRegisterModel,
        callee_saved: &'model BTreeSet<RegisterUnitId>,
    ) -> Self {
        Self {
            physical,
            callee_saved,
            functions: Vec::new(),
            function_count: 0,
            block_count: 0,
            instruction_count: 0,
            operand_count: 0,
            write_count: 0,
            witness_count: 0,
        }
    }

    pub(super) fn scan_instruction(
        &mut self,
        machine: psi_core::MachineId,
        block: SelectedBlockId,
        instruction: &SelectedInstruction,
        homes: &omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
        units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) -> Result<(), AllocatedCalleeSavedRequirementError> {
        self.instruction_count = add(self.instruction_count, 1)?;
        for operand in &instruction.operands {
            self.operand_count = add(self.operand_count, 1)?;
            if !matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            ) {
                continue;
            }
            let mut matching = homes
                .assignments
                .iter()
                .filter(|home| home.virtual_register == operand.virtual_register);
            let home =
                matching
                    .next()
                    .ok_or(AllocatedCalleeSavedRequirementError::MissingHome {
                        function: machine,
                        virtual_register: operand.virtual_register,
                    })?;
            if matching.next().is_some() {
                return Err(AllocatedCalleeSavedRequirementError::DuplicateHome {
                    function: machine,
                    virtual_register: operand.virtual_register,
                });
            }
            let view = self
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
            let witness = CalleeSavedModificationWitness::OperandDefinition {
                block,
                instruction: instruction.id,
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                home_view: home.view,
                write_semantics: view.write_semantics,
            };
            self.scan_units(&view.write_units, witness, units)?;
        }
        self.scan_units(
            &instruction.implicit_defs,
            CalleeSavedModificationWitness::ImplicitDefinition {
                block,
                instruction: instruction.id,
            },
            units,
        )?;
        self.scan_units(
            &instruction.clobbers,
            CalleeSavedModificationWitness::ImplicitClobber {
                block,
                instruction: instruction.id,
            },
            units,
        )
    }

    pub(super) fn scan_implicit(
        &mut self,
        block: SelectedBlockId,
        instruction: omega_selected_instructions::SelectedInstructionId,
        definitions: &[RegisterUnitId],
        clobbers: &[RegisterUnitId],
        units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) -> Result<(), AllocatedCalleeSavedRequirementError> {
        self.instruction_count = add(self.instruction_count, 1)?;
        self.scan_units(
            definitions,
            CalleeSavedModificationWitness::ImplicitDefinition { block, instruction },
            units,
        )?;
        self.scan_units(
            clobbers,
            CalleeSavedModificationWitness::ImplicitClobber { block, instruction },
            units,
        )
    }

    fn scan_units(
        &mut self,
        writes: &[RegisterUnitId],
        witness: CalleeSavedModificationWitness,
        units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) -> Result<(), AllocatedCalleeSavedRequirementError> {
        for unit in writes {
            self.write_count = add(self.write_count, 1)?;
            if self.callee_saved.contains(unit) {
                self.witness_count = add(self.witness_count, 1)?;
                units.entry(*unit).or_default().push(witness);
            }
        }
        Ok(())
    }
}

pub(super) fn finish_units(
    units: BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
) -> Vec<AllocatedCalleeSavedUnitRequirement> {
    units
        .into_iter()
        .map(|(unit, witnesses)| AllocatedCalleeSavedUnitRequirement { unit, witnesses })
        .collect()
}

pub(super) fn add(value: u64, increment: u64) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    value
        .checked_add(increment)
        .ok_or(AllocatedCalleeSavedRequirementError::WorkOverflow)
}
