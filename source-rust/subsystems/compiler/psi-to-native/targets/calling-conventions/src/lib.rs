/*
We normalize target calling conventions, boundary value classification,
aggregate and Terminal storage layouts, callback materialization plans, and
host operation bindings into one deterministic backend contract. Our `CallPlan`,
`StatePlan`, and `BoundaryEntryPlan` vocabulary in `plans` defines how values
travel across registers, outgoing stack slots, and caller-owned indirect
copies, while `boundary_values` and `structural_shapes` classify public and
structural value graphs under Microsoft x64, System V AMD64, AAPCS64, and Linux
syscall policies.
*/

/*
We separate these normalized contracts and source-free value graphs from both
source-authored policy evaluation and target instruction emission so provider
realization, foreign admission, and native lowering consume one canonical
representation without re-running source evaluators or drifting on register
banks, eightbyte classes, or callback slot identities. Emitted footprint and
generated-entry evidence remain distinct from boundary contract identity so
register allocation or code-generation changes revalidate an implementation
without altering its published requirement commitment.
*/

/*
Start in `plans.rs` for the calling-plan, machine-state, and entry-evidence
contracts, `boundary_values.rs` and `structural_shapes.rs` for recursive ABI
value classification, and `aggregate_layout.rs` plus `terminal_layout.rs` for
record and conventional sum storage geometry. Read `boundary_application.rs`
and `callback_materializations.rs` for native parameter telescopes, callback
slot commitments, and registrar binder joins, and `host_operations.rs` for our
host capability and operation catalog, external binding rows, Darwin and
Windows import library tables, and x86-64 interrupt gate address assembly.
*/

pub(crate) use ::wire::codec_bytes;
pub(crate) use ::wire::digest_encoder;
mod aggregate_layout;
mod borrowed_storage;
mod boundary_application;
mod boundary_values;
mod callback_materializations;
pub mod frame_unwind;
mod host_operations;
mod plans;
pub mod process_entry;
mod structural_shapes;
pub mod terminal_layout;
#[cfg(test)]
mod tests;
pub use structural_shapes::{InvalidStructuralShape, classified_boundary_shape};

pub use aggregate_layout::{
    ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout,
    evaluate_conventional_sum_layout,
};
pub use boundary_application::{
    BoundaryCallbackBinding, BoundaryCallbackLayout, BoundaryCallingApplication,
    BoundaryCallingApplicationCandidate, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterShape, BoundaryOpaqueApplication, BoundarySignatureGeometry,
    CallbackDataGeometry, CallbackInlineFieldGeometry, CallbackSlotGeometry, NativeCallProjection,
    NativeParameterTelescope, PrivateCallbackPlacements, TerminalCallingTelescope,
    callback_destination_slot, encode_private_callback_placements,
    validate_fresh_native_parameter_identity,
};
pub use boundary_values::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryValueClass, BoundaryValueField,
    BoundaryValueGraph, BoundaryValueShape,
};
pub use callback_materializations::{
    BoundaryNativeParameterOrigin, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, LayoutPlanId, LayoutSlotId,
    NativeCallbackDemand, NativeParameterApplication, NativeParameterId, NativePlace,
    StaticMachineBinderId, callback_layout_field_slot_id, callback_layout_plan_id,
    callback_layout_slot_id, callback_nominal_report_fingerprint, callback_plan_laid_layout_id,
    callback_requirement_id, nominal_callback_native_parameter_id,
};
pub use host_operations::{
    DARWIN_LIBOBJC_PATH, ExternalBindingKind, ExternalBindingRow, FunctionFieldLocation,
    HostCapability, HostOperation, InterruptGateAddress, darwin_import_library,
    interrupt_gate_address, windows_import_library,
};
pub use plans::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, GeneratedEntryControl,
    GeneratedEntryEvidence, GeneratedEntryInstruction, IndirectPointerLocation, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, PlanDiagnostic, Preemption, RegisterSet,
    StateEffectsEvidence, StateFootprintEvidence, StatePlan, SystemVEightbyteClass,
    ValidatedBoundaryEntryPlan, ValueClass, ValueLocation, ValuePlacement, ValueShape,
    byte_size_at_offset_zero, encode_state_plan_identity, evaluate_call_plan,
    evaluate_ordinary_boundary_entry_plan, gpr_code, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};

/// The admitted foreign ABI and scalar parameters required by one native adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridgeRequest {
    pub target: target::NativeTarget, // Target platform for the foreign call adapter.
    pub foreign: ValidatedBoundaryEntryPlan, // Admitted native C boundary entry plan.
    pub scalar_parameters: Vec<Option<semantic_vocabulary::ScalarType>>, // Per-parameter scalar types for narrow sign extension.
}

/// Identifies an already published adapter for a host call. This value carries
/// no executable memory or library bytes. Only the sponsor owns those.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedForeignAdapterIdentity {
    pub bridge: NativeForeignBridgeRequest,
    pub locator: target::NormalizedForeignLocator,
    pub application: [u8; 32],
    pub image_digest: [u8; 32],
}
