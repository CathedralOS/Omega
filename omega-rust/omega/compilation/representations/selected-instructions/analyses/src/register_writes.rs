//! Read physical register writes from current instructions and assignments.
//!
//! Allocation derives its preservation candidate with this traversal; explicit
//! replay joins the original function roster independently before comparing the
//! candidate. These are witnesses and work counts, not allocation authority.

pub use error::AllocatedCalleeSavedRequirementError;
use register_homes::{
    AllocatedCalleeSavedUnitRequirement, CalleeSavedModificationWitness,
    FunctionAllocatedCalleeSavedRequirements,
};
pub use state::DirectTraversal;
pub use work::{extent, usage};

pub(crate) mod state {
    use std::collections::{BTreeMap, BTreeSet};

    use register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{SelectedBlockId, SelectedInstruction, terminator_instruction};

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedUnitRequirement,
        CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    };

    pub(crate) struct HomeIndex<'homes> {
        sorted: Vec<&'homes register_homes::VirtualRegisterHome>,
    }

    impl<'homes> HomeIndex<'homes> {
        pub(crate) fn new(
            homes: &'homes register_homes::FunctionRegisterHomes,
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

        fn unique(
            &self,
            machine: semantic_vocabulary::MachineId,
            register: selected_instructions::VirtualRegisterId,
        ) -> Result<&'homes register_homes::VirtualRegisterHome, AllocatedCalleeSavedRequirementError>
        {
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

    pub struct DirectTraversal<'model> {
        pub(crate) physical: &'model ValidatedPhysicalRegisterModel,
        pub(crate) callee_saved: &'model BTreeSet<RegisterUnitId>,
        pub(crate) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
        pub(crate) function_count: u64,
        pub(crate) block_count: u64,
        pub(crate) instruction_count: u64,
        pub(crate) operand_count: u64,
        pub(crate) write_count: u64,
        pub(crate) witness_count: u64,
    }

    impl<'model> DirectTraversal<'model> {
        pub fn new(
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

        pub fn into_functions(self) -> Vec<FunctionAllocatedCalleeSavedRequirements> {
            self.functions
        }

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

        pub fn walk_function(
            &mut self,
            function: &selected_instructions::SelectedFunction,
            homes: &register_homes::FunctionRegisterHomes,
        ) -> Result<(), AllocatedCalleeSavedRequirementError> {
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
                    self.scan_instruction(
                        function.machine,
                        block.id,
                        instruction,
                        &homes,
                        &mut units,
                    )?;
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

        pub(crate) fn scan_implicit(
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

        pub(crate) fn scan_units(
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

    pub(crate) fn add(
        value: u64,
        increment: u64,
    ) -> Result<u64, AllocatedCalleeSavedRequirementError> {
        value
            .checked_add(increment)
            .ok_or(AllocatedCalleeSavedRequirementError::WorkOverflow)
    }
}
pub(crate) mod work {
    use optimization_core::OptimizationWorkUsage;

    use super::{
        AllocatedCalleeSavedRequirementError,
        state::{DirectTraversal, add},
    };

    pub fn usage(
        traversal: &DirectTraversal<'_>,
    ) -> Result<OptimizationWorkUsage, AllocatedCalleeSavedRequirementError> {
        let modified_units = traversal
            .functions
            .iter()
            .try_fold(0_u64, |total, function| {
                add(
                    total,
                    u64::try_from(function.modified_units.len())
                        .map_err(|_| AllocatedCalleeSavedRequirementError::WorkOverflow)?,
                )
            })?;
        let function_block_instruction_operand_writes = extent(traversal)?;
        Ok(OptimizationWorkUsage {
            rule_evaluations: add(traversal.function_count, 1)?,
            candidates: modified_units,
            validation_steps: function_block_instruction_operand_writes,
            commits: [
                1,
                traversal.function_count,
                modified_units,
                traversal.witness_count,
            ]
            .into_iter()
            .try_fold(0_u64, add)?,
            iterations: function_block_instruction_operand_writes,
        })
    }

    pub fn extent(
        traversal: &DirectTraversal<'_>,
    ) -> Result<u64, AllocatedCalleeSavedRequirementError> {
        [
            traversal.function_count,
            traversal.block_count,
            traversal.instruction_count,
            traversal.operand_count,
            traversal.write_count,
        ]
        .into_iter()
        .try_fold(0_u64, add)
    }
}

pub(crate) mod error {
    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_model::RegisterViewId;
    use selected_instructions::VirtualRegisterId;
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum AllocatedCalleeSavedRequirementError {
        RootMismatch,
        UnsupportedPolicy,
        UnsupportedTargetConvention,
        FunctionRosterMismatch,
        HomeRosterMismatch,
        DuplicateHome {
            function: MachineId,
            virtual_register: VirtualRegisterId,
        },
        MissingHome {
            function: MachineId,
            virtual_register: VirtualRegisterId,
        },
        UnknownOrIncompatibleView {
            function: MachineId,
            virtual_register: VirtualRegisterId,
            view: RegisterViewId,
        },
        NonCanonicalRequirements,
        UsageMismatch,
        WorkOverflow,
        BudgetExceeded {
            required: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        },
    }

    impl std::fmt::Display for AllocatedCalleeSavedRequirementError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "allocated callee-saved requirement derivation failed: {self:?}"
            )
        }
    }
    impl std::error::Error for AllocatedCalleeSavedRequirementError {}
}
