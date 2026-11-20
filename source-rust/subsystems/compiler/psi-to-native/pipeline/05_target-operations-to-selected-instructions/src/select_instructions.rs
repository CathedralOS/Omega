//! Select target instructions for one prepared body and its admitted installations.
#![forbid(unsafe_code)]

mod input;
mod installation_resources;
mod instruction_selection;
mod selected;

#[cfg(feature = "test-support")]
pub mod test_support;

use input::BoundExecutionPlan;
pub use input::SelectionInput;
pub use selected::{OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions};

/// Select instructions for a prepared execution plan.
pub fn select_instructions(
    input: SelectionInput,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (program, publication, private_references) = input.body.into_selection();
    let semantic = &publication.facts;
    let target = program.plan.plan();
    if target.psi != semantic.program.psi || target.entry != semantic.program.entry {
        return Err(OptimizedSelectionPipelineError::BodyBindings(
            "physical planning inputs name different programs or entries",
        ));
    }
    let installations = crate::installation_resources::InstallationRequirements {
        interrupts: semantic.admission.installed_interrupts(),
        callbacks: semantic.admission.native_callbacks(),
        entry_execution: input.entry_execution,
        stack: input.stack,
    }
    .bind(&program.plan)
    .map_err(OptimizedSelectionPipelineError::Installation)?;
    let register_environment =
        register_environment::baseline_target_register_environment(target.target)
            .map_err(OptimizedSelectionPipelineError::RegisterEnvironment)?;
    let bound = BoundExecutionPlan::bind(&program.plan, installations.facts())
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    let selected = instruction_selection::select_bound_instructions(&bound, &register_environment)
        .map_err(OptimizedSelectionPipelineError::Selection)?;
    let custody =
        selected::selection_custody_receipt(&program, &register_environment, &bound, &selected);
    Ok(StagedOptimizedSelectedInstructions {
        body: std::sync::Arc::new(
            physical_program::NativeBodyFacts::retain_selected_body_facts(
                publication,
                private_references,
                installations,
            ),
        ),
        register_environment,
        selections: program.selections,
        budget_per_pass: program.budget_per_pass,
        selected,
        custody,
    })
}
