#![forbid(unsafe_code)]

pub mod installation_resources;
mod instruction_selection;

pub use instruction_selection::{
    OptimizedSelectionPipelineError, SelectionInput, StagedOptimizedSelectedInstructions,
    stage_optimized_instruction_selection,
};

#[cfg(feature = "test-support")]
pub use instruction_selection::{
    BoundInstallations, InstallationBindingError, select_instructions_for_plan,
};
