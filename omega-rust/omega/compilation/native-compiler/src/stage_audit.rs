// The stage checks machine emission runs when a realized function is audited.
// Only this driver may call Omega stages 05 to 09, so it hands each realization
// a `NativeStageAudit` that runs their validators: the current-form checks of
// stages 07, 08 and 09 always, and, when a validator request retained them, the
// comparison with the original allocation, machine plan, encoding and baseline
// layout. Machine emission reads the results through
// `machine_emission::FunctionStageAudit` and names no stage.

use machine_emission::{FunctionStageAudit, StageAuditError};
use optimization_core::OptimizationExecutionPhase;
use post_allocation_machine_to_selected_form_encoding::{
    SelectedFormEncodingValidation, StagedOptimizedSelectedFormEncoding,
    validate_current_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use resolved_layout_to_resolved_layout::{
    StagedOptimizedX86BranchRelaxation, validate_current_layout_optimization,
    validate_resolved_layout_optimization,
};
use selected_form_encoding_to_resolved_layout::{
    ResolvedLayoutValidation, StagedOptimizedResolvedSelectedFormLayout,
    validate_current_resolved_layout, validate_optimized_resolved_selected_form_layout,
};
use selected_instruction_analyses::{RetainedSelectedOptimizationRun, SelectedOptimizationRecord};
use selected_instructions_to_register_homes::{
    RetainedAllocation, SelectedProgramView, validate_allocated_callee_saved_requirements,
};

#[derive(Debug)]
pub(crate) struct NativeStageAudit {
    /// Omega 09's relaxation trace, which its current-form check reads.
    relaxation: Option<StagedOptimizedX86BranchRelaxation>,
    originals: Option<RetainedOriginals>,
}

/// The original stage outputs a validator request keeps for replay.
#[derive(Debug)]
pub(crate) struct RetainedOriginals {
    pub allocation: RetainedAllocation,
    pub machine: StagedOptimizedPostAllocationMachinePlan,
    pub encoding: StagedOptimizedSelectedFormEncoding,
    pub baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    /// The Omega 04 rewrite run behind the allocated program, when a rewrite
    /// phase ran. Only its own validator reads it.
    pub selected_optimization: Option<Box<dyn RetainedSelectedOptimizationRun>>,
}

impl NativeStageAudit {
    pub(crate) fn new(
        relaxation: Option<StagedOptimizedX86BranchRelaxation>,
        originals: Option<RetainedOriginals>,
    ) -> Self {
        Self {
            relaxation,
            originals,
        }
    }
}

fn refused(stage: &'static str, error: impl std::fmt::Debug) -> StageAuditError {
    StageAuditError::Stage {
        stage,
        detail: format!("{error:?}"),
    }
}

impl FunctionStageAudit for NativeStageAudit {
    fn validate_current(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<(), StageAuditError> {
        let program = &layout.program;
        validate_current_selected_form_encoding(
            &program.instructions,
            &program.frame,
            &program.encoding,
        )
        .map_err(|error| refused("Omega 07 selected-form encoding", error))?;
        validate_current_resolved_layout(program)
            .map_err(|error| refused("Omega 08 resolved layout", error))?;
        validate_current_layout_optimization(layout, self.relaxation.as_ref())
            .map_err(|error| refused("Omega 09 layout optimization", error))
    }

    fn verify_originals(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<register_homes::AllocatedCalleeSavedRequirements, StageAuditError> {
        let originals = self
            .originals
            .as_ref()
            .ok_or(StageAuditError::ReplayUnavailable)?;
        originals
            .allocation
            .validate_publication()
            .map_err(|error| refused("Omega 05 allocation publication", error))?;
        let program = &layout.program;
        let current = originals.allocation.current();
        let selected = current.selected();
        let environment = current.register_environment();
        let physical = environment.physical();
        let replay = post_allocation_machine::program_validation::PhysicalProgramReplay {
            selected: selected.current_program(),
            machine: originals.machine.machine().plan(),
            environment,
            preservation: originals.allocation.preservation().retained(),
            publication: originals.allocation.publication(),
            phase: post_allocation_machine::PostAllocationMachinePhaseReceipt::from_parts(
                originals.machine.optimization().complete_selection(),
                originals.machine.optimization().phase_selections(),
                originals.machine.optimization().result(),
            ),
        };
        post_allocation_machine::program_validation::validate_current_program(
            &program.instructions,
            replay,
        )
        .map_err(StageAuditError::CurrentProgram)?;
        if program.encoding.as_ref() != originals.encoding.program()
            || layout.baseline.identity != originals.baseline_layout.identity()
        {
            return Err(StageAuditError::RootMismatch);
        }
        validate_optimized_layout_independent_selected_form_encoding(
            SelectedFormEncodingValidation {
                replay,
                frame: Some(&program.frame),
                artifact: &originals.encoding,
            },
        )
        .map_err(|error| refused("Omega 07 selected-form encoding", error))?;
        validate_optimized_resolved_selected_form_layout(ResolvedLayoutValidation {
            replay,
            pre_layout: originals.encoding.program(),
            artifact: &originals.baseline_layout,
        })
        .map_err(|error| refused("Omega 08 resolved layout", error))?;
        validate_resolved_layout_optimization(
            physical,
            originals.baseline_layout.program(),
            &current
                .selections()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
            layout,
            self.relaxation.as_ref(),
        )
        .map_err(|error| refused("Omega 09 layout optimization", error))?;
        let requirements = validate_allocated_callee_saved_requirements(
            &current,
            program.instructions.preservation.plan().clone(),
        )
        .map_err(|error| refused("Omega 05 callee-saved requirements", error))?;
        Ok(requirements.retained().clone())
    }

    fn has_originals(&self) -> bool {
        self.originals.is_some()
    }

    fn selected_optimization(
        &self,
    ) -> Option<(
        Option<&dyn RetainedSelectedOptimizationRun>,
        &SelectedOptimizationRecord,
    )> {
        let originals = self.originals.as_ref()?;
        Some((
            originals.selected_optimization.as_deref(),
            originals.allocation.current().history(),
        ))
    }
}
