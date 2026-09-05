//! Cross-stage realization and independent replay after pipeline-owner consolidation.
#[path = "pipeline_ownership/mod.rs"]
mod tests;

use abstract_operations_to_target_operations::*;
use machine_emission::*;
use native_artifact::*;
use object_file::*;
use optimization_core::OptimizationReportRequest;
use post_allocation_machine_to_frame_layout::*;
use post_allocation_machine_to_post_allocation_machine::*;
use post_allocation_machine_to_selected_form_encoding::*;
use register_homes_to_post_allocation_machine::*;
use selected_form_encoding_to_resolved_layout::*;
use selected_instructions_to_register_homes::*;
use target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, validate_optimized_selection_custody,
};
use target_to_register_environment::*;
use terminal_psi_to_native_artifact::*;
