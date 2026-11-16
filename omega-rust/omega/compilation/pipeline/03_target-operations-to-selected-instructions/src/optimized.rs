/*
We coordinate the `Omega 03` optimized instruction-selection pipeline
(`stage_optimized_instruction_selection`), binding the validated target register
environment, target-operation legalization, and instruction selection into one
custody-checked `StagedOptimizedSelectedInstructions` product.
*/

pub(crate) mod constraints {
    use crate::BoundExecutionPlan;
    use selected_instructions::{SelectedFixedInputConstraint, SelectedSelectionConstraints};
    use semantic_vocabulary::MachineId;
    use target_vocabulary::MachineRegister;

    use register_environment::ValidatedTargetRegisterEnvironment;

    pub(crate) fn selection_constraints(
        legalized: &BoundExecutionPlan<'_>,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> SelectedSelectionConstraints {
        let mut fixed_inputs = Vec::new();
        for function in &legalized.plan().scalar_functions {
            let required = crate::selection::value_transport::required_values(function);
            for (index, parameter) in function.parameters.iter().enumerate() {
                if !required.contains(&parameter.value) {
                    continue;
                }
                if let [
                    calling_conventions::ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: _,
                    },
                ] = parameter.placement.locations.as_slice()
                {
                    push_fixed_input(
                        &mut fixed_inputs,
                        environment,
                        function.machine,
                        parameter.value,
                        index,
                        *register,
                    );
                }
            }
        }
        SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs,
        }
    }

    fn push_fixed_input(
        inputs: &mut Vec<SelectedFixedInputConstraint>,
        environment: &ValidatedTargetRegisterEnvironment,
        machine: MachineId,
        source_value: semantic_vocabulary::ValueId,
        parameter_index: usize,
        register: MachineRegister,
    ) {
        if inputs.iter().any(|input| {
            input.machine == machine
                && input.source_value == source_value
                && input.parameter_index == parameter_index
                && input.register == register
        }) {
            return;
        }
        let Some(fixed_view) = environment.fixed_register_view(register) else {
            return;
        };
        inputs.push(SelectedFixedInputConstraint {
            machine,
            source_value,
            parameter_index,
            register,
            fixed_view,
        });
    }
}
pub(crate) mod construction {
    use crate::{
        BoundExecutionPlan, ValidatedSelectedInstructions, selection::select_instructions,
    };
    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::constraints::selection_constraints;
    use super::model::OptimizedSelectionPipelineError;

    pub(crate) fn construct_optimized_instruction_selection(
        bound: &BoundExecutionPlan<'_>,
        register_environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<ValidatedSelectedInstructions, OptimizedSelectionPipelineError> {
        let selection_constraints = selection_constraints(bound, register_environment);
        select_instructions(bound, &selection_constraints, register_environment)
            .map_err(OptimizedSelectionPipelineError::Selection)
    }
}
pub(crate) mod custody {
    use crate::{BoundExecutionPlan, ValidatedSelectedInstructions};
    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::input::SelectionInput;
    use super::model::StagedOptimizedSelectionCustodyReceipt;

    use selected_instructions::StagedOptimizedCustodyCore;

    pub(crate) fn selection_custody_receipt(
        input: &SelectionInput<'_>,
        register_environment: &ValidatedTargetRegisterEnvironment,
        bound: &BoundExecutionPlan<'_>,
        selected: &ValidatedSelectedInstructions,
    ) -> StagedOptimizedSelectionCustodyReceipt {
        let plan = bound.plan();
        StagedOptimizedSelectionCustodyReceipt {
            core: StagedOptimizedCustodyCore {
                psi: plan.psi,
                target: plan.target,
                entry: plan.entry,
                optimization: input.optimization,
                projection: input.projection,
                manifest: input.manifest,
                optimization_unit: plan.optimization_unit,
                work_schedule: plan.work_schedule,
                register_environment: register_environment.identity(),
            },
            legalized: bound.receipt().identity(),
            legalization_validator: bound.receipt().validator(),
            selected: selected.receipt().identity(),
            function_count: selected.plan().functions.len(),
        }
    }
}
pub(crate) mod input {
    use legalized_operations::{LegalizedOperationPlan, PreparationReceipt, PreparedExecutionPlan};
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationWorkBudget,
        OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
    };

    /// What instruction selection is given: the execution plan stage 02
    /// published, the installations the driver bound to it, and the
    /// optimization identities the custody receipt records. No source module,
    /// abstract program or optimization-unit table is part of this input.
    #[derive(Debug)]
    pub struct SelectionInput<'input> {
        pub plan: &'input PreparedExecutionPlan,
        pub installations: BoundInstallations<'input>,
        pub optimization: OptimizationIdentityBundleIdentity,
        pub projection: OptimizedAbstractPlanProjectionIdentity,
        pub manifest: PrePhysicalOptimizationManifestIdentity,
        pub selections: OptimizationSelections,
        pub budget_per_pass: OptimizationWorkBudget,
    }

    /// Installation authority the driver resolved after preparation: the
    /// admitted interrupt cohort, the installed entry, and the stack each
    /// installed handler needs. It grants permission and deployment
    /// resources; the plan alone says what the program does.
    #[derive(Debug, Clone, Copy)]
    pub struct BoundInstallations<'input> {
        pub interrupts: &'input provider_admission::AdmittedInstalledInterruptCohort,
        pub entry_execution: Option<&'input provider_admission::AdmittedInstalledEntryExecution>,
        pub stack_requirements:
            &'input [installation_evidence::InstalledInterruptStackRequirement],
    }

    /// Why a set of installations cannot be used with a plan.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InstallationBindingError {
        InterruptCohortProgram,
        EntryExecution,
        StackRequirement,
    }

    /// A plan together with installations that were checked to belong to it.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct BoundExecutionPlan<'input> {
        prepared: &'input PreparedExecutionPlan,
        installations: BoundInstallations<'input>,
    }

    impl<'input> BoundExecutionPlan<'input> {
        /// Installations name the program, target and entry they were
        /// admitted for. Refuse any that name another plan.
        pub(crate) fn bind(
            prepared: &'input PreparedExecutionPlan,
            installations: BoundInstallations<'input>,
        ) -> Result<Self, InstallationBindingError> {
            let plan = prepared.plan();
            if installations.interrupts.psi() != plan.psi {
                return Err(InstallationBindingError::InterruptCohortProgram);
            }
            if let Some(execution) = installations.entry_execution {
                let entry = execution.entry().entry();
                if entry.psi() != plan.psi
                    || entry.target() != plan.target
                    || entry.machine() != plan.entry
                {
                    return Err(InstallationBindingError::EntryExecution);
                }
            }
            if installations
                .stack_requirements
                .iter()
                .any(|requirement| requirement.psi != plan.psi || requirement.target != plan.target)
            {
                return Err(InstallationBindingError::StackRequirement);
            }
            Ok(Self {
                prepared,
                installations,
            })
        }

        pub(crate) fn plan(&self) -> &'input LegalizedOperationPlan {
            self.prepared.plan()
        }

        pub(crate) fn receipt(&self) -> PreparationReceipt {
            self.prepared.receipt()
        }

        pub(crate) fn installed_interrupts(
            &self,
        ) -> &'input provider_admission::AdmittedInstalledInterruptCohort {
            self.installations.interrupts
        }

        pub(crate) fn installed_entry_execution(
            &self,
        ) -> Option<&'input provider_admission::AdmittedInstalledEntryExecution> {
            self.installations.entry_execution
        }

        pub(crate) fn installed_stack_requirements(
            &self,
        ) -> &'input [installation_evidence::InstalledInterruptStackRequirement] {
            self.installations.stack_requirements
        }
    }
}
pub(crate) mod model {
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
        OptimizationValidatorIdentity, OptimizationWorkBudget,
        OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
    };
    use selected_instructions::SelectedInstructionPlanIdentity;
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use terminal_psi::TerminalPsiIdentity;

    use crate::{SelectedInstructionError, ValidatedSelectedInstructions};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use selected_instructions::StagedOptimizedCustodyCore;

    #[derive(Debug)]
    pub struct StagedOptimizedSelectedInstructions {
        pub(crate) register_environment: ValidatedTargetRegisterEnvironment,
        pub(crate) selections: OptimizationSelections,
        pub(crate) budget_per_pass: OptimizationWorkBudget,
        pub(crate) selected: ValidatedSelectedInstructions,
        pub(crate) custody: StagedOptimizedSelectionCustodyReceipt,
    }

    impl StagedOptimizedSelectedInstructions {
        pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
            &self.register_environment
        }

        pub const fn selected(&self) -> &ValidatedSelectedInstructions {
            &self.selected
        }

        pub fn selected_program(&self) -> selected_instructions::SelectedProgram {
            let receipt = self.selected.receipt();
            selected_instructions::SelectedProgram {
                plan: self.selected.shared_plan(),
                identity: receipt.identity(),
                optimization_unit: receipt.optimization_unit(),
                work_schedule: receipt.work_schedule(),
            }
        }

        pub const fn selections(&self) -> &OptimizationSelections {
            &self.selections
        }

        pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
            self.budget_per_pass
        }

        pub const fn custody(&self) -> StagedOptimizedSelectionCustodyReceipt {
            self.custody
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedSelectionCustodyReceipt {
        pub(crate) core: StagedOptimizedCustodyCore,
        pub(crate) legalized: legalized_operations::LegalizedOperationPlanIdentity,
        pub(crate) legalization_validator: OptimizationValidatorIdentity,
        pub(crate) selected: SelectedInstructionPlanIdentity,
        pub(crate) function_count: usize,
    }

    impl StagedOptimizedSelectionCustodyReceipt {
        selected_instructions::staging_custody_core_accessors!();

        pub const fn selected(self) -> SelectedInstructionPlanIdentity {
            self.selected
        }

        pub const fn legalized(self) -> legalized_operations::LegalizedOperationPlanIdentity {
            self.legalized
        }

        pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
            self.legalization_validator
        }

        pub const fn function_count(self) -> usize {
            self.function_count
        }
    }

    #[derive(Debug)]
    pub enum OptimizedSelectionPipelineError {
        Binding(super::input::InstallationBindingError),
        Selection(SelectedInstructionError),
    }

    failed_display!(
        OptimizedSelectionPipelineError,
        "optimized instruction selection failed"
    );

    impl std::error::Error for OptimizedSelectionPipelineError {}
}

pub(crate) use input::BoundExecutionPlan;
pub use input::{BoundInstallations, InstallationBindingError, SelectionInput};
pub use model::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt,
};

use register_environment::ValidatedTargetRegisterEnvironment;

/// Select instructions from a prepared plan and its bound installations
/// alone, without the optimization identities a staged product records.
///
/// This is the whole of what selection needs about the program. Stage 02's
/// tests call it to show that a plan they prepared can be selected with the
/// source module, the abstract program and the optimization unit out of reach.
pub fn select_instructions_for_plan(
    plan: &legalized_operations::PreparedExecutionPlan,
    installations: BoundInstallations<'_>,
    register_environment: &ValidatedTargetRegisterEnvironment,
) -> Result<crate::ValidatedSelectedInstructions, OptimizedSelectionPipelineError> {
    let bound = BoundExecutionPlan::bind(plan, installations)
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    construction::construct_optimized_instruction_selection(&bound, register_environment)
}

/// Select instructions for a prepared execution plan.
pub fn stage_optimized_instruction_selection(
    input: SelectionInput<'_>,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let bound = BoundExecutionPlan::bind(input.plan, input.installations)
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    let selected =
        construction::construct_optimized_instruction_selection(&bound, &register_environment)?;
    let custody =
        custody::selection_custody_receipt(&input, &register_environment, &bound, &selected);
    Ok(StagedOptimizedSelectedInstructions {
        register_environment,
        selections: input.selections,
        budget_per_pass: input.budget_per_pass,
        selected,
        custody,
    })
}
