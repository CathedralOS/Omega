/*
We derive and independently replay `ValidatedAllocatedCalleeSavedRequirements`
by intersecting each function's assigned physical register write units,
implicit definitions, and clobbers with the target ABI's callee-saved register
roster.
*/

pub(crate) mod compute {
    pub(crate) mod ordinary {
        use super::{super::AllocatedCalleeSavedRequirementError, state::DirectTraversal};

        pub(crate) fn derive(
            traversal: &mut DirectTraversal<'_>,
            selected: &selected_instructions::SelectedFunctions,
            homes: &[crate::FunctionRegisterHomes],
        ) -> Result<(), AllocatedCalleeSavedRequirementError> {
            if selected.len() != homes.len() {
                return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
            }
            for (function, homes) in selected.iter().zip(homes) {
                traversal.walk_function(function, homes)?;
            }
            Ok(())
        }
    }
    pub(crate) mod state {
        use std::collections::{BTreeMap, BTreeSet};

        use register_model::{
            RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel,
        };
        use selected_instructions::{SelectedBlockId, SelectedInstruction, terminator_instruction};

        use super::super::{
            AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedUnitRequirement,
            CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
        };

        pub(crate) struct HomeIndex<'homes> {
            sorted: Vec<&'homes crate::VirtualRegisterHome>,
        }

        impl<'homes> HomeIndex<'homes> {
            pub(crate) fn new(
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

            fn unique(
                &self,
                machine: semantic_vocabulary::MachineId,
                register: selected_instructions::VirtualRegisterId,
            ) -> Result<&'homes crate::VirtualRegisterHome, AllocatedCalleeSavedRequirementError>
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

        pub(crate) struct DirectTraversal<'model> {
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
            pub(crate) fn new(
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

            pub(crate) fn walk_function(
                &mut self,
                function: &selected_instructions::SelectedFunction,
                homes: &crate::FunctionRegisterHomes,
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
            super::AllocatedCalleeSavedRequirementError,
            state::{DirectTraversal, add},
        };

        pub(crate) fn usage(
            traversal: &DirectTraversal<'_>,
        ) -> Result<OptimizationWorkUsage, AllocatedCalleeSavedRequirementError> {
            let modified_units =
                traversal
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

        pub(crate) fn extent(
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

    use std::collections::BTreeSet;

    use optimization_core::OptimizationWorkBudget;

    use crate::AllocationOutput;
    use register_environment::selected_abi_preservation;
    use selected_instruction_analyses::SelectedProgramView;

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
        AllocatedCalleeSavedRequirementPolicy,
    };
    use state::DirectTraversal;

    pub(crate) fn derive(
        source: &AllocationOutput<'_>,
        policy: AllocatedCalleeSavedRequirementPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementError> {
        if policy
            != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
        {
            return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
        }
        let selected = source.selected();
        let environment = source.register_environment();
        let homes = source.homes();
        let manifest = source.post_allocation_manifest().record();
        if manifest.selected != selected.selected_identity()
            || manifest.homes != homes.receipt().identity()
            || manifest.register_environment != environment.identity()
            || manifest.target != environment.target()
        {
            return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
        }
        let preservation = selected_abi_preservation(environment)
            .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
        let callee_saved = preservation
            .convention
            .callee_saved
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut traversal = DirectTraversal::new(environment.physical(), &callee_saved);
        ordinary::derive(
            &mut traversal,
            &selected.plan().functions,
            &homes.plan().functions,
        )?;
        let usage = work::usage(&traversal)?;
        let ceiling = budget.for_extent(work::extent(&traversal)?);
        if !usage.within(ceiling) {
            return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
                required: usage,
                budget: ceiling,
            });
        }
        Ok(AllocatedCalleeSavedRequirementPlan {
            selected: selected.selected_identity(),
            homes: homes.receipt().identity(),
            post_allocation_manifest: manifest.identity,
            register_environment: environment.identity(),
            physical_register_model: environment.physical().identity(),
            target: environment.target(),
            abi: preservation.kind,
            callee_saved_units: preservation.convention.callee_saved.clone(),
            policy,
            budget,
            usage,
            functions: traversal.functions,
        })
    }
}
pub(crate) mod custody {
    use super::{
        AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementReceipt,
        allocated_callee_saved_requirement_identity,
    };

    pub(crate) fn seal(
        plan: &AllocatedCalleeSavedRequirementPlan,
    ) -> AllocatedCalleeSavedRequirementReceipt {
        AllocatedCalleeSavedRequirementReceipt {
            identity: allocated_callee_saved_requirement_identity(plan),
        }
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

    stage_error_display!(
        AllocatedCalleeSavedRequirementError,
        "allocated callee-saved requirement derivation failed"
    );
}
pub(crate) mod identity {
    use super::{
        AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
        AllocatedCalleeSavedRequirementPolicy, encode_callee_saved_modification_witness_identity,
    };

    pub(crate) fn allocated_callee_saved_requirement_identity(
        plan: &AllocatedCalleeSavedRequirementPlan,
    ) -> AllocatedCalleeSavedRequirementIdentity {
        let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.allocated-callee-saved-requirements.v2",
        );
        hasher.raw(&plan.selected.bytes());
        hasher.raw(&plan.homes.bytes());
        hasher.raw(&plan.post_allocation_manifest.bytes());
        hasher.raw(&plan.register_environment.bytes());
        hasher.raw(&plan.physical_register_model.bytes());
        hasher.raw(&plan.target.identity_bytes());
        hasher.byte(plan.abi.abi_tag());
        register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
        hasher.byte(match plan.policy {
            AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1 => 0,
        });
        hasher.raw(&plan.budget.encode());
        hasher.raw(&plan.usage.encode());
        register_model::sha256_length(&mut hasher, plan.functions.len());
        for function in &plan.functions {
            hasher.raw(&function.machine.get().to_le_bytes());
            register_model::sha256_length(&mut hasher, function.modified_units.len());
            for requirement in &function.modified_units {
                hasher.raw(&requirement.unit.0.to_le_bytes());
                register_model::sha256_length(&mut hasher, requirement.witnesses.len());
                for witness in &requirement.witnesses {
                    encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
                }
            }
        }
        AllocatedCalleeSavedRequirementIdentity::from_bytes(hasher.finish())
    }
}
pub(crate) mod replay {
    pub(crate) mod state {
        use std::collections::BTreeMap;

        use semantic_vocabulary::MachineId;

        pub(crate) use super::super::compute::state::DirectTraversal as ReplayTraversal;

        #[allow(clippy::needless_lifetimes)]
        pub(crate) fn keyed_homes<'home>(
            functions: &'home [crate::FunctionRegisterHomes],
        ) -> Result<
            BTreeMap<MachineId, &'home crate::FunctionRegisterHomes>,
            super::super::AllocatedCalleeSavedRequirementError,
        > {
            let mut keyed = BTreeMap::new();
            for function in functions {
                if keyed.insert(function.machine, function).is_some() {
                    return Err(
                        super::super::AllocatedCalleeSavedRequirementError::FunctionRosterMismatch,
                    );
                }
            }
            Ok(keyed)
        }
    }

    use std::collections::BTreeSet;

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

    use crate::AllocationOutput;
    use register_environment::{FrameAbiPreservationConvention, selected_abi_preservation};

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPolicy,
        FunctionAllocatedCalleeSavedRequirements, compute::work,
    };
    use state::{ReplayTraversal, keyed_homes};

    pub(crate) struct ReplayResult {
        pub(crate) abi: FrameAbiPreservationConvention,
        pub(crate) callee_saved_units: Vec<register_model::RegisterUnitId>,
        pub(crate) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
        pub(crate) usage: OptimizationWorkUsage,
    }

    pub(crate) fn reconstruct(
        source: &AllocationOutput<'_>,
        policy: AllocatedCalleeSavedRequirementPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<ReplayResult, AllocatedCalleeSavedRequirementError> {
        if policy
            != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
        {
            return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
        }
        let selected = source.selected_plan();
        let environment = source.register_environment();
        let preservation = selected_abi_preservation(environment)
            .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
        let callee_saved = preservation
            .convention
            .callee_saved
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let ordinary_homes = keyed_homes(&source.homes().plan().functions)?;
        if ordinary_homes.len() != selected.functions.len() {
            return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
        }
        let mut traversal = ReplayTraversal::new(environment.physical(), &callee_saved);
        for function in &selected.functions {
            let homes = ordinary_homes
                .get(&function.machine)
                .ok_or(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch)?;
            traversal.walk_function(function, homes)?;
        }
        let usage = work::usage(&traversal)?;
        let ceiling = budget.for_extent(work::extent(&traversal)?);
        if !usage.within(ceiling) {
            return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
                required: usage,
                budget: ceiling,
            });
        }
        Ok(ReplayResult {
            abi: preservation.kind,
            callee_saved_units: preservation.convention.callee_saved.clone(),
            functions: traversal.functions,
            usage,
        })
    }
}
pub(crate) mod validation {
    use crate::AllocationOutput;
    use selected_instruction_analyses::SelectedProgramView;

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
        ValidatedAllocatedCalleeSavedRequirements, custody, replay,
    };

    pub fn validate_allocated_callee_saved_requirements(
        current: &AllocationOutput<'_>,
        candidate: AllocatedCalleeSavedRequirementPlan,
    ) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError>
    {
        let environment = current.register_environment();
        let manifest = current.post_allocation_manifest().record();
        if candidate.selected != current.selected().selected_identity()
            || candidate.homes != current.homes().receipt().identity()
            || candidate.post_allocation_manifest != manifest.identity
            || candidate.register_environment != environment.identity()
            || candidate.physical_register_model != environment.physical().identity()
            || candidate.target != environment.target()
        {
            return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
        }
        let replayed = replay::reconstruct(current, candidate.policy, candidate.budget)?;
        if candidate.usage != replayed.usage {
            return Err(AllocatedCalleeSavedRequirementError::UsageMismatch);
        }
        if candidate.abi != replayed.abi
            || candidate.callee_saved_units != replayed.callee_saved_units
            || candidate.functions != replayed.functions
        {
            return Err(AllocatedCalleeSavedRequirementError::NonCanonicalRequirements);
        }
        let receipt = custody::seal(&candidate);
        Ok(ValidatedAllocatedCalleeSavedRequirements {
            plan: candidate,
            receipt,
        })
    }
}

pub use error::AllocatedCalleeSavedRequirementError;
pub(crate) use identity::allocated_callee_saved_requirement_identity;
pub use validation::validate_allocated_callee_saved_requirements;

use crate::AllocationOutput;
use optimization_core::OptimizationWorkBudget;
pub use register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity,
};

pub fn stage_allocated_callee_saved_requirements(
    current: &AllocationOutput<'_>,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let plan = compute::derive(current, policy, budget)?;
    let receipt = custody::seal(&plan);
    Ok(ValidatedAllocatedCalleeSavedRequirements { plan, receipt })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementReceipt {
    pub(crate) identity: AllocatedCalleeSavedRequirementIdentity,
}

impl AllocatedCalleeSavedRequirementReceipt {
    receipt_accessor_rows!(AllocatedCalleeSavedRequirementReceipt {
        identity: AllocatedCalleeSavedRequirementIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatedCalleeSavedRequirements {
    pub(crate) plan: AllocatedCalleeSavedRequirementPlan,
    pub(crate) receipt: AllocatedCalleeSavedRequirementReceipt,
}

impl ValidatedAllocatedCalleeSavedRequirements {
    receipt_accessor_rows!(borrow ValidatedAllocatedCalleeSavedRequirements {
        plan: &AllocatedCalleeSavedRequirementPlan,
    });
    receipt_accessor_rows!(ref ValidatedAllocatedCalleeSavedRequirements {
        receipt: AllocatedCalleeSavedRequirementReceipt,
    });
}
