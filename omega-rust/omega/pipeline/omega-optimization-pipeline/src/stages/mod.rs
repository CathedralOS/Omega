//! Optimizer module role: stage group. Ordered custody boundaries from target selection through native artifacts.

pub(crate) mod artifacts;
pub(crate) mod encoding;
pub(crate) mod layout;
pub(crate) mod machine;
pub(crate) mod realization;

pub use artifacts::*;
pub use encoding::*;
pub use layout::*;
pub use machine::*;
pub use omega_abstract_operations_to_target_operations::{
    ValidatedOptimizedTargetOperations, lower_optimized_to_target_operations,
    lower_optimized_to_target_operations_with_ieee_float_fma_settlements,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};
pub use omega_allocation_legality_to_active_resident_rematerialization::*;
pub use omega_allocation_legality_to_fixed_view_copies::*;
pub use omega_allocation_legality_to_literal_folds::*;
pub use omega_allocation_legality_to_register_homes::*;
pub use omega_callee_saved_requirements_to_save_storage::*;
pub use omega_fixed_view_copies_to_reanalyzed_legality::*;
pub use omega_literal_folds_to_register_homes::*;
pub use omega_live_ranges_to_allocation_legality::*;
pub use omega_liveness_to_live_ranges::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use omega_register_homes_to_callee_saved_requirements::*;
pub use omega_selected_instructions_to_liveness::*;
pub use omega_selected_instructions_to_machine_effects::*;
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
