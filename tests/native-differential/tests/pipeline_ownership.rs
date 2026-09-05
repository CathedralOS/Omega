//! Cross-stage realization and independent replay after pipeline-owner consolidation.
#[path = "pipeline_ownership/mod.rs"]
mod tests;

use abstract_operations_to_target_operations::*;
use machine_emission::frame_layout::*;
use machine_emission::*;
use native_artifact::*;
use native_realization::*;
use object_file::*;
use optimization_core::OptimizationReportRequest;
use post_allocation_machine_to_post_allocation_machine::*;
use post_allocation_machine_to_resolved_layout::selected_form_encoding::*;
use post_allocation_machine_to_resolved_layout::*;
use register_environment::*;
use register_homes_to_post_allocation_machine::*;
use selected_instructions_to_register_homes::*;
use target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, validate_optimized_selection_custody,
};
