mod artifacts;
mod callback_installation;
mod callback_placements;
mod callback_private_relocations;
mod callback_root_schedule;
mod plan;
mod timing;

pub use artifacts::BackendArtifactRoots;
#[cfg(feature = "test-support")]
pub use callback_installation::test_support::callback_installation_test_fixture;
pub use callback_installation::{
    CallbackInstallationEntry, CallbackInstallationManifest, build_callback_installation_manifest,
};
pub use callback_placements::{
    BoundCallbackPrivateMaterialization, BoundNominalCallbackPlacement,
    CallbackPlacementBindingIdentity, CallbackThunkPlan, callback_placement_binding_identity,
    callback_thunk_placement_identity_fingerprint, canonical_callback_private_symbol,
    validate_bound_nominal_callback_placement,
};
pub use callback_private_relocations::{
    CallbackPrivateObjectStoreRequest, CallbackPrivateRelocationDemand,
    CallbackRegistrarArgumentBinding, CallbackRegistrarAssignedOperandBinding,
    CallbackRegistrarPhysicalDestination, CallbackRegistrarPhysicalDestinationKind,
    replay_callback_private_object_store_requests, replay_callback_private_relocation_demand,
    replay_callback_private_relocation_demands, replay_callback_registrar_argument_bindings,
    replay_callback_registrar_assigned_operand_bindings,
    replay_callback_registrar_physical_destinations,
};
pub use callback_root_schedule::{
    CallbackRootActivationIdentity, CallbackRootSchedule, plan_callback_root_schedule,
    replay_callback_root_schedule,
};
pub use plan::BackendPlan;
pub use timing::BackendPlanPhaseTiming;
