//! Optimizer module role: stage group. Ordered custody boundaries from target selection through native artifacts.

pub(crate) mod artifacts;
pub(crate) mod layout;
pub(crate) mod realization;

pub use artifacts::*;
pub use layout::*;
pub use omega_abstract_operations_to_target_operations::{
    ValidatedOptimizedTargetOperations, lower_optimized_to_target_operations,
    lower_optimized_to_target_operations_with_ieee_float_fma_settlements,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};
pub use omega_callee_saved_requirements_to_save_storage::*;
pub use omega_frame_layout_to_frame_protocol::*;
pub use omega_post_allocation_machine_to_frame_layout::*;
pub use omega_post_allocation_machine_to_optimized_machine::*;
pub use omega_post_allocation_machine_to_selected_form_encoding::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use omega_register_homes_to_callee_saved_requirements::*;
pub use omega_register_homes_to_post_allocation_machine::*;
pub use omega_selected_instructions_to_machine_effects::*;
pub use omega_selected_instructions_to_register_homes::*;
pub use omega_spill_access_constraints_to_frame_requirements::*;
pub use omega_target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    validate_optimized_selection_custody,
};
pub use omega_target_to_register_environment::{
    FrameAbiPreservationConvention, TargetRegisterEnvironmentValidationError,
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    validate_target_register_environment, validate_target_register_environment_with_reservations,
};
pub use realization::*;
