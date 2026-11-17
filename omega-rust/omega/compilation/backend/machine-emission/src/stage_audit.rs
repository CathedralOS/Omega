/*
An audit of a realized function checks it against the stages that produced it:
the current encoding, layout and layout optimization against their own
validators, and, on a validator request, the current program against the
original allocation, machine plan, encoding and baseline layout. Those
validators belong to Omega stages 05 to 09, which only the native driver may
call. The driver therefore hands each realization a `FunctionStageAudit` that
runs them, and this library reads the result without naming a stage. A default
compile attaches an audit holding only what the current-form checks need; the
originals stay out of it.
*/

use selected_instruction_analyses::{RetainedSelectedOptimizationRun, SelectedOptimizationRecord};

/// The stage checks for one realized function, supplied by the native driver.
pub trait FunctionStageAudit: std::fmt::Debug + Send + Sync {
    /// Check the current encoding, layout and layout optimization with the
    /// validators of the stages that produced them.
    fn validate_current(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<(), StageAuditError>;

    /// Compare the current program with the retained original allocation,
    /// machine plan, encoding and baseline layout, and return the callee-saved
    /// requirements re-derived from the original allocation. Refuses with
    /// `ReplayUnavailable` when the compile did not retain the originals.
    fn verify_originals(
        &self,
        layout: &machine_code::OptimizedMachineLayout,
    ) -> Result<register_homes::AllocatedCalleeSavedRequirements, StageAuditError>;

    /// Whether the compile retained the originals `verify_originals` reads.
    fn has_originals(&self) -> bool;

    /// The retained Omega 04 rewrite run, if one produced this program, and the
    /// record the allocated program carries; `None` when the originals were
    /// not retained.
    fn selected_optimization(
        &self,
    ) -> Option<(
        Option<&dyn RetainedSelectedOptimizationRun>,
        &SelectedOptimizationRecord,
    )>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageAuditError {
    /// The realization carries no stage checks; its driver attached none.
    ChecksUnavailable,
    /// The compile did not retain the originals this audit compares with.
    ReplayUnavailable,
    /// The current program differs from the original allocation and machine plan.
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    /// The current form names different original roots.
    RootMismatch,
    /// A stage validator refused the current form or its originals.
    Stage { stage: &'static str, detail: String },
}
