use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
use selected_instructions::{SelectedBlockId, SelectedInstruction, terminator_instruction};

use super::super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
};

/// One function's home assignments sorted by virtual register, so each
/// written operand finds its home by binary search instead of rescanning every
/// assignment in the function. A register with two homes has no single home a
/// write can name, so the index rejects a repeated home for the whole roster
/// when it is built — the only owner of that admission, whichever route reads
/// it.
pub(super) struct HomeIndex<'homes> {
    sorted: Vec<&'homes crate::VirtualRegisterHome>,
}

impl<'homes> HomeIndex<'homes> {
    pub(super) fn new(
        homes: &'homes crate::FunctionRegisterHomes,
    ) -> Result<Self, AllocatedCalleeSavedRequirementError> {
        let mut sorted = homes.assignments.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|home| home.virtual_register);
        if let Some(repeated) = sorted
            .windows(2)
            .find(|pair| pair[0].virtual_register == pair[1].virtual_register)
        {
            return Err(AllocatedCalleeSavedRequirementError::DuplicateHome {
                function: homes.machine,
                virtual_register: repeated[0].virtual_register,
            });
        }
        Ok(Self { sorted })
    }

    /// The one home of `register`; a missing home rejects.
    fn unique(
        &self,
        machine: semantic_vocabulary::MachineId,
        register: selected_instructions::VirtualRegisterId,
    ) -> Result<&'homes crate::VirtualRegisterHome, AllocatedCalleeSavedRequirementError> {
        let start = self
            .sorted
            .partition_point(|home| home.virtual_register < register);
        match self.sorted.get(start) {
            Some(home) if home.virtual_register == register => Ok(home),
            _ => Err(AllocatedCalleeSavedRequirementError::MissingHome {
                function: machine,
                virtual_register: register,
            }),
        }
    }
}

/// One callee-saved walk's state: the physical model and roster it reads, the
/// requirements it derives, and the counts the pass budget is charged for.
/// The direct and replay routes share it and the walk it runs; they differ
/// only in the function roster each one hands in, never in what a step counts
/// or admits.
pub(in crate::preservation) struct DirectTraversal<'model> {
    pub(in crate::preservation) physical: &'model ValidatedPhysicalRegisterModel,
    pub(in crate::preservation) callee_saved: &'model BTreeSet<RegisterUnitId>,
    pub(in crate::preservation) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
    pub(in crate::preservation) function_count: u64,
    pub(in crate::preservation) block_count: u64,
    pub(in crate::preservation) instruction_count: u64,
    pub(in crate::preservation) operand_count: u64,
    pub(in crate::preservation) write_count: u64,
    pub(in crate::preservation) witness_count: u64,
}

impl<'model> DirectTraversal<'model> {
    pub(in crate::preservation) fn new(
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

    /// Close one function's walk: its recorded unit witnesses become the
    /// requirement row the plan publishes.
    fn finish(
        &mut self,
        machine: semantic_vocabulary::MachineId,
        units: BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) {
        self.functions
            .push(FunctionAllocatedCalleeSavedRequirements {
                machine,
                modified_units: units
                    .into_iter()
                    .map(|(unit, witnesses)| AllocatedCalleeSavedUnitRequirement {
                        unit,
                        witnesses,
                    })
                    .collect(),
            });
    }

    /// One function's whole walk: its home roster, every block, each block's
    /// instructions and terminator, and the unit writes they record. The
    /// direct and the replay route both run this; they differ only in the
    /// roster each one hands in, never in what a step counts or admits.
    pub(in crate::preservation) fn walk_function(
        &mut self,
        function: &selected_instructions::SelectedFunction,
        homes: &crate::FunctionRegisterHomes,
    ) -> Result<(), AllocatedCalleeSavedRequirementError> {
        // The allocation owns home completeness, including omitted dead
        // parameters. Every actual write is still checked against its home
        // below.
        if function.machine != homes.machine {
            return Err(AllocatedCalleeSavedRequirementError::HomeRosterMismatch);
        }
        let homes = HomeIndex::new(homes)?;
        self.function_count = add(self.function_count, 1)?;
        let mut units = BTreeMap::new();
        for block in &function.blocks {
            self.block_count = add(self.block_count, 1)?;
            for instruction in block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
            {
                self.scan_instruction(function.machine, block.id, instruction, &homes, &mut units)?;
            }
        }
        self.finish(function.machine, units);
        Ok(())
    }

    fn scan_instruction(
        &mut self,
        machine: semantic_vocabulary::MachineId,
        block: SelectedBlockId,
        instruction: &SelectedInstruction,
        homes: &HomeIndex<'_>,
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
            let home = homes.unique(machine, operand.virtual_register)?;
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
        self.scan_implicit(
            block,
            instruction.id,
            &instruction.implicit_defs,
            &instruction.clobbers,
            units,
        )
    }

    /// Charge one instruction's implicit definitions and clobbers: the unit rows
    /// that follow from the instruction itself, whichever route found the
    /// operand homes.
    pub(in crate::preservation) fn scan_implicit(
        &mut self,
        block: SelectedBlockId,
        instruction: selected_instructions::SelectedInstructionId,
        definitions: &[RegisterUnitId],
        clobbers: &[RegisterUnitId],
        units: &mut BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) -> Result<(), AllocatedCalleeSavedRequirementError> {
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

    /// Charge one write and, when it meets the ABI-preserved roster, record the
    /// witness that made it a requirement.
    pub(in crate::preservation) fn scan_units(
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

pub(in crate::preservation) fn add(
    value: u64,
    increment: u64,
) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    value
        .checked_add(increment)
        .ok_or(AllocatedCalleeSavedRequirementError::WorkOverflow)
}
