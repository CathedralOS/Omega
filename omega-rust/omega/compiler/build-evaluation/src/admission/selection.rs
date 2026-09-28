//! Build root extraction and the realization-owned entry contracts it feeds.

pub(crate) mod root_bindings;

pub(crate) use provider_planning::program_entry::ProductEntryQueryCompatibility;
pub use provider_planning::program_entry::{
    SelectedCompilerProgramEntry, SelectedProgramEntry, SelectedProgramEntryCallingPlans,
    program_entry_semantic_binding_role, select_compiler_program_entry,
    selected_program_entry_machine, validate_selected_program_entry_calling_plan,
    validate_selected_program_entry_shape,
};
