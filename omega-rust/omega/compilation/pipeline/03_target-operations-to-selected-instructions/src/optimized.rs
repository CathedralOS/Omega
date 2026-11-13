/*
We coordinate the `Omega 03` optimized instruction-selection pipeline
(`stage_optimized_instruction_selection`), binding the validated target register
environment, target-operation legalization, and instruction selection into one
custody-checked `StagedOptimizedSelectedInstructions` product.
*/

pub(crate) mod constraints {
    use crate::ValidatedLegalizedOperations;
    use selected_instructions::{SelectedFixedInputConstraint, SelectedSelectionConstraints};
    use semantic_vocabulary::MachineId;
    use target_operations::MachineRegister;

    use register_environment::ValidatedTargetRegisterEnvironment;

    pub(crate) fn selection_constraints(
        legalized: &ValidatedLegalizedOperations,
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
        ValidatedLegalizedOperations, ValidatedSelectedInstructions, legalize_target_operations,
        select_instructions,
    };
    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::constraints::selection_constraints;
    use super::input::TargetSelectionInput;
    use super::model::OptimizedSelectionPipelineError;

    pub(crate) fn construct_optimized_instruction_selection(
        input: &TargetSelectionInput<'_>,
        register_environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<
        (ValidatedLegalizedOperations, ValidatedSelectedInstructions),
        OptimizedSelectionPipelineError,
    > {
        let legalized = legalize_target_operations(
            input.target,
            input.source,
            input.unit,
            input.origins,
            input.callable_roots,
            input.installed_interrupts,
            input.installed_entry_execution,
            input.installed_stack_requirements,
        )
        .map_err(OptimizedSelectionPipelineError::Legalization)?;
        let selection_constraints = selection_constraints(&legalized, register_environment);
        let selected =
            select_instructions(&legalized, &selection_constraints, register_environment)
                .map_err(OptimizedSelectionPipelineError::Selection)?;
        Ok((legalized, selected))
    }
}
pub(crate) mod custody {
    use crate::{ValidatedLegalizedOperations, ValidatedSelectedInstructions};
    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::input::TargetSelectionInput;
    use super::model::StagedOptimizedSelectionCustodyReceipt;

    use selected_instructions::StagedOptimizedCustodyCore;

    pub(crate) fn selection_custody_receipt(
        input: &TargetSelectionInput<'_>,
        register_environment: &ValidatedTargetRegisterEnvironment,
        legalized: &ValidatedLegalizedOperations,
        selected: &ValidatedSelectedInstructions,
    ) -> StagedOptimizedSelectionCustodyReceipt {
        let (target, unit) = (input.target, input.unit);
        StagedOptimizedSelectionCustodyReceipt {
            core: StagedOptimizedCustodyCore {
                psi: target.psi,
                target: target.target,
                entry: target.entry,
                optimization: input.optimization,
                projection: input.projection,
                manifest: input.manifest,
                optimization_unit: unit.identity,
                work_schedule: unit.work_schedule,
                register_environment: register_environment.identity(),
            },
            legalized: legalized.receipt().identity(),
            legalization_validator: legalized.receipt().validator(),
            selected: selected.receipt().identity(),
            function_count: selected.plan().functions.len(),
        }
    }
}
pub(crate) mod input {
    use abstract_operations::AbstractOperationPlan;
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationWorkBudget,
        OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
    };
    use optimization_unit::PsiOptimizationUnit;
    use target_operations::{BoundaryOriginFacts, TargetOperationPlan};

    #[derive(Debug)]
    pub struct TargetSelectionInput<'input> {
        pub target: &'input TargetOperationPlan,
        pub source: &'input AbstractOperationPlan,
        pub unit: &'input PsiOptimizationUnit,
        pub origins: BoundaryOriginFacts<'input>,
        pub callable_roots: &'input terminal_semantics::callable_values::CallableInvocationRoots,
        pub installed_entry_execution:
            Option<&'input provider_admission::AdmittedInstalledEntryExecution>,
        pub installed_interrupts: &'input provider_admission::AdmittedInstalledInterruptCohort,
        pub installed_stack_requirements:
            &'input [installation_evidence::InstalledInterruptStackRequirement],
        pub optimization: OptimizationIdentityBundleIdentity,
        pub projection: OptimizedAbstractPlanProjectionIdentity,
        pub manifest: PrePhysicalOptimizationManifestIdentity,
        pub selections: OptimizationSelections,
        pub budget_per_pass: OptimizationWorkBudget,
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

    use crate::{LegalizationError, SelectedInstructionError, ValidatedSelectedInstructions};
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
        Legalization(LegalizationError),
        Selection(SelectedInstructionError),
    }

    failed_display!(
        OptimizedSelectionPipelineError,
        "optimized instruction selection failed"
    );

    impl std::error::Error for OptimizedSelectionPipelineError {}
}

pub use input::TargetSelectionInput;
pub use model::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt,
};

use register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_optimized_instruction_selection(
    input: TargetSelectionInput<'_>,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (legalized, selected) =
        construction::construct_optimized_instruction_selection(&input, &register_environment)?;
    let custody =
        custody::selection_custody_receipt(&input, &register_environment, &legalized, &selected);
    Ok(StagedOptimizedSelectedInstructions {
        register_environment,
        selections: input.selections,
        budget_per_pass: input.budget_per_pass,
        selected,
        custody,
    })
}
