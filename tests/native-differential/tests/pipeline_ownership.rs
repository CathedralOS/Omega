//! Cross-stage realization and independent replay after pipeline-owner consolidation.
#[path = "pipeline_ownership/mod.rs"]
mod tests;

use omega_abstract_operations_to_target_operations::*;
use omega_callee_saved_requirements_to_save_storage::*;
use omega_frame_layout_to_frame_protocol::*;
use omega_machine_emission::*;
use omega_native_artifact::*;
use omega_object_file::*;
use omega_optimization_core::OptimizationReportRequest;
use omega_post_allocation_machine_to_frame_layout::*;
use omega_post_allocation_machine_to_optimized_machine::*;
use omega_post_allocation_machine_to_selected_form_encoding::*;
use omega_register_homes_to_callee_saved_requirements::*;
use omega_register_homes_to_post_allocation_machine::*;
use omega_selected_form_encoding_to_resolved_layout::*;
use omega_selected_instructions_to_machine_effects::*;
use omega_selected_instructions_to_register_homes::*;
use omega_spill_access_constraints_to_frame_requirements::*;
use omega_target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, validate_optimized_selection_custody,
};
use omega_target_to_register_environment::*;
use omega_terminal_psi_to_native_artifact::*;
