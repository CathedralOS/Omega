//! Verify current physical forms and explicitly retained originals.
//!
//! Evidence is concrete immutable data. No executable compiler callback is
//! attached to a program, and a missing requested original remains a refusal.

use encoding_admission::layout::{
    ResolvedLayoutValidation, validate_current_resolved_layout,
    validate_optimized_resolved_selected_form_layout,
};
use encoding_admission::relaxation::{
    validate_current_layout_optimization, validate_resolved_layout_optimization,
};
use encoding_admission::{
    SelectedFormEncodingValidation, validate_encoded_integrity,
    validate_optimized_layout_independent_selected_form_encoding,
};
use optimization_core::OptimizationExecutionPhase;
use physical_planning::RetainedAllocation;
use physical_planning::StagedOptimizedPostAllocationMachinePlan;
use native_layout::StagedOptimizedX86BranchRelaxation;
use selected_instruction_analyses::SelectedOptimizationRecord;
mod allocation;
use allocation::AllocationOriginals;
use allocation::validate_allocated_callee_saved_requirements;
use selected_instruction_analyses::SelectedProgramView;
pub use physical_planning::SelectedOptimizationReplay;

#[derive(Debug)]
pub struct PhysicalVerificationEvidence {
    /// Omega 09's relaxation trace, which its current-form check reads.
    relaxation: Option<StagedOptimizedX86BranchRelaxation>,
    originals: Option<RetainedPhysicalInputs>,
}

/// The original stage outputs a validator request keeps for replay.
#[derive(Debug)]
pub struct RetainedPhysicalInputs {
    pub physical: post_allocation_machine::PostAllocationMachineProgram,
    pub allocation: RetainedAllocation,
    pub machine: StagedOptimizedPostAllocationMachinePlan,
    pub encoding: machine_code::EncodedMachineProgram,
    pub baseline_layout: machine_code::ResolvedMachineProgram,
    /// The Omega 04 rewrite run behind the allocated program, when a rewrite
    /// phase ran. Only its own validator reads it.
    pub selected_optimization: Option<SelectedOptimizationReplay>,
}

impl PhysicalVerificationEvidence {
    pub fn new(
        relaxation: Option<StagedOptimizedX86BranchRelaxation>,
        originals: Option<RetainedPhysicalInputs>,
    ) -> Self {
        Self {
            relaxation,
            originals,
        }
    }
}

fn refused(stage: &'static str, error: impl std::fmt::Debug) -> VerificationError {
    VerificationError::Stage {
        stage,
        detail: format!("{error:?}"),
    }
}

impl PhysicalVerificationEvidence {
    pub fn validate_current(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<(), VerificationError> {
        let program = &layout.program;
        validate_encoded_integrity(&program.context, &program.frame, &program.encoding)
            .map_err(|error| refused("Omega 07 selected-form encoding", error))?;
        validate_current_resolved_layout(program)
            .map_err(|error| refused("Omega 08 resolved layout", error))?;
        validate_current_layout_optimization(
            layout,
            self.relaxation.as_ref().map(|issued| issued.record()),
        )
        .map_err(|error| refused("Omega 09 layout optimization", error))
    }

    pub fn verify_originals(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<register_homes::AllocatedCalleeSavedRequirements, VerificationError> {
        let originals = self
            .originals
            .as_ref()
            .ok_or(VerificationError::ReplayUnavailable)?;
        originals
            .allocation
            .validate_publication()
            .map_err(|error| refused("Omega 05 allocation publication", error))?;
        let program = &layout.program;
        let current = &AllocationOriginals::retain(&originals.allocation);
        current
            .validate_roots()
            .map_err(|error| refused("Omega 05 allocation roots", error))?;
        let selected = &current.selected;
        let environment = &current.environment;
        let physical = environment.physical();
        let replay = post_allocation_machine::program_validation::PhysicalProgramReplay {
            selected: selected.current_program(),
            machine: originals.machine.machine().plan(),
            environment,
            preservation: &current.preservation,
            publication: &current.publication,
            phase: post_allocation_machine::PostAllocationMachinePhaseReceipt::from_parts(
                originals.machine.optimization().complete_selection(),
                originals.machine.optimization().phase_selections(),
                originals.machine.optimization().result(),
            ),
        };
        post_allocation_machine::program_validation::validate_current_program(
            &originals.physical,
            replay,
        )
        .map_err(VerificationError::CurrentProgram)?;
        if program.context != originals.encoding.context
            || program.layout_requirements != originals.encoding.layout_requirements
            || program.exit_requirements != originals.encoding.exit_requirements
            || program.fragment_origins != originals.encoding.fragment_origins
            || program.frame_demands != originals.encoding.frame_demands
            || program.encoding.as_ref() != originals.encoding.encoding.as_ref()
            || layout.baseline.identity != originals.baseline_layout.layout.identity()
        {
            return Err(VerificationError::RootMismatch);
        }
        validate_optimized_layout_independent_selected_form_encoding(
            SelectedFormEncodingValidation {
                physical: &originals.physical,
                replay,
                frame: Some(&program.frame),
                artifact: &originals.encoding,
            },
        )
        .map_err(|error| refused("Omega 07 selected-form encoding", error))?;
        validate_optimized_resolved_selected_form_layout(ResolvedLayoutValidation {
            physical: &originals.physical,
            replay,
            pre_layout: originals.encoding.encoding.as_ref(),
            artifact: &originals.baseline_layout,
        })
        .map_err(|error| refused("Omega 08 resolved layout", error))?;
        validate_resolved_layout_optimization(
            physical,
            &originals.baseline_layout.layout,
            &current
                .publication
                .selections()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
            layout,
            self.relaxation.as_ref().map(|issued| issued.record()),
        )
        .map_err(|error| refused("Omega 09 layout optimization", error))?;
        let requirements = validate_allocated_callee_saved_requirements(
            current,
            program.context.preservation.plan().clone(),
        )
        .map_err(|error| refused("Omega 05 callee-saved requirements", error))?;
        Ok(requirements)
    }

    pub fn original_physical_program(
        &self,
    ) -> Option<&post_allocation_machine::PostAllocationMachineProgram> {
        self.originals.as_ref().map(|originals| &originals.physical)
    }
    pub fn has_originals(&self) -> bool {
        self.originals.is_some()
    }

    pub fn selected_optimization(
        &self,
    ) -> Option<(
        Option<&SelectedOptimizationReplay>,
        &SelectedOptimizationRecord,
    )> {
        let originals = self.originals.as_ref()?;
        Some((
            originals.selected_optimization.as_ref(),
            originals.allocation.current().history(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// The compile did not retain the originals this audit compares with.
    ReplayUnavailable,
    /// The current program differs from the original allocation and machine plan.
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    /// The current form names different original roots.
    RootMismatch,
    /// A stage validator refused the current form or its originals.
    Stage { stage: &'static str, detail: String },
}
pub mod bridges;
