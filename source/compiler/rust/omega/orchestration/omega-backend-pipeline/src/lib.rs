use omega_core::parallel::WorkerPoolHandle;
use omega_target::TargetProfile;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

use omega_control_flow::ControlFlowPlan;

mod builder;
mod callback_private_object_stores;
mod callback_private_relocations;
mod callback_registrar_arguments;
mod callback_registrar_assigned_operands;
mod callback_registrar_destinations;
mod callback_thunks;
mod entry;
mod program_storage_wrapper;
mod skeleton;
mod timing;

pub use builder::render_frame_slot_table;
pub use omega_backend_plan::{BackendPlan, BackendPlanPhaseTiming};
pub use program_storage_wrapper::{
    ProgramStorageEntryWrapperInsertion, insert_program_storage_entry_wrapper,
};

/// `freestanding`: the selected build trusts no ambient host packages, so use
/// an empty host ABI baseline (no implicit bindings/lowerings or import
/// thunks). This policy is independent of image subsystem metadata.
pub fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    selected_provider_plans: Arc<omega_effects::SelectedProviderPlanFacts>,
    entry_machine_name: Option<&str>,
    entry_boundary_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    callback_placements: Arc<[omega_backend_plan::BoundNominalCallbackPlacement]>,
    target_profile: TargetProfile,
    freestanding: bool,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    builder::build_backend_plan_from_control_flow_with_workers(
        program,
        selected_provider_plans,
        entry_machine_name,
        entry_boundary_plan,
        callback_placements,
        target_profile,
        freestanding,
        external_binding_rows,
        control_flow,
        workers,
    )
}
