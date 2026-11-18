/*
We coordinate the `Omega 03` optimized instruction-selection pipeline
(`stage_optimized_instruction_selection`), binding the validated target register
environment, target-operation legalization, and instruction selection into one
custody-checked `StagedOptimizedSelectedInstructions` product.
*/

pub(crate) mod constraints {
    use crate::instruction_selection::BoundExecutionPlan;
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
            let required =
                crate::instruction_selection::selection::value_transport::required_values(function);
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
    use crate::instruction_selection::{
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
    use crate::instruction_selection::{BoundExecutionPlan, ValidatedSelectedInstructions};
    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::input::SelectionInput;
    use super::model::StagedOptimizedSelectionCustodyReceipt;

    use selected_instructions::StagedOptimizedCustodyCore;

    pub(crate) fn selection_custody_receipt(
        input: &SelectionInput,
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
                optimization: input.program.optimization,
                projection: input.program.projection,
                manifest: input.program.manifest,
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

    #[derive(Debug)]
    pub struct SelectionInput {
        pub program: legalized_operations::PreparedInstructionSelection,
        pub installations: std::sync::Arc<BoundInstallations>,
    }

    /// Installation authority the driver projected from admission after
    /// preparation: the interrupt installations, the installed entry, and
    /// the stack each installed handler needs. It grants permission and
    /// deployment resources; the plan alone says what the program does.
    pub use bound_installations::BoundInstallations;

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
        installations: &'input BoundInstallations,
    }

    impl<'input> BoundExecutionPlan<'input> {
        /// Installations name the program, target and entry they were
        /// admitted for. Refuse any that name another plan.
        pub(crate) fn bind(
            prepared: &'input PreparedExecutionPlan,
            installations: &'input BoundInstallations,
        ) -> Result<Self, InstallationBindingError> {
            let plan = prepared.plan();
            if installations.psi != plan.psi {
                return Err(InstallationBindingError::InterruptCohortProgram);
            }
            if installations.installed_entry.is_some_and(|entry| {
                entry.psi != plan.psi || entry.target != plan.target || entry.machine != plan.entry
            }) {
                return Err(InstallationBindingError::EntryExecution);
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

        pub(crate) fn callable_invocations(
            &self,
        ) -> &'input terminal_semantics::callable_values::ReachableCallableInvocations {
            self.prepared.callable_invocations()
        }

        pub(crate) fn receipt(&self) -> PreparationReceipt {
            self.prepared.receipt()
        }

        pub(crate) fn installations(&self) -> &'input BoundInstallations {
            self.installations
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

    use crate::instruction_selection::{SelectedInstructionError, ValidatedSelectedInstructions};
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
        pub fn selected_program(&self) -> selected_instructions::SelectedProgram {
            let receipt = self.selected.receipt();
            selected_instructions::SelectedProgram {
                plan: self.selected.shared_plan(),
                identity: receipt.identity(),
                optimization_unit: receipt.optimization_unit(),
                work_schedule: receipt.work_schedule(),
            }
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
pub use input::SelectionInput;
#[cfg(feature = "test-support")]
pub use input::{BoundInstallations, InstallationBindingError};
pub use model::{OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions};

use register_environment::ValidatedTargetRegisterEnvironment;

/// Select instructions from a prepared plan and its bound installations
/// alone, without the optimization identities a staged product records.
///
/// This is the whole of what selection needs about the program. Stage 02's
/// tests call it to show that a plan they prepared can be selected with the
/// source module, the abstract program and the optimization unit out of reach.
#[cfg(feature = "test-support")]
pub fn select_instructions_for_plan(
    plan: &legalized_operations::PreparedExecutionPlan,
    installations: &BoundInstallations,
    register_environment: &ValidatedTargetRegisterEnvironment,
) -> Result<
    crate::instruction_selection::ValidatedSelectedInstructions,
    OptimizedSelectionPipelineError,
> {
    let bound = BoundExecutionPlan::bind(plan, installations)
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    construction::construct_optimized_instruction_selection(&bound, register_environment)
}

/// Select instructions for a prepared execution plan.
pub fn stage_optimized_instruction_selection(
    input: SelectionInput,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let bound = BoundExecutionPlan::bind(&input.program.plan, &input.installations)
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    let selected =
        construction::construct_optimized_instruction_selection(&bound, &register_environment)?;
    let custody =
        custody::selection_custody_receipt(&input, &register_environment, &bound, &selected);
    Ok(StagedOptimizedSelectedInstructions {
        register_environment,
        selections: input.program.selections,
        budget_per_pass: input.program.budget_per_pass,
        selected,
        custody,
    })
}
