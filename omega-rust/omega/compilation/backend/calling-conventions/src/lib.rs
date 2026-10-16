//! Calling conventions: value passing, stack realization, and callbacks per host.
//!
//! `plans` owns the calling-plan vocabulary the README's contract publishes;
//! `darwin`, `linux`, and `windows` fix each host's registers, syscall numbers,
//! and import libraries; `aggregate_layout` decides how aggregates and stack
//! slots are passed; `callback_materializations` binds native callbacks.
//! Layout and selection consume these; they never re-derive them.
//!
//! Start at `host_operations.rs`: the host operation catalog, its
//! capabilities and the external binding rows that bind an operation to a
//! foreign symbol; `plans` owns the calling-plan vocabulary,
//! `callback_materializations` realizes callbacks,
//! `boundary_values` classifies public value graphs and checks authored ABI
//! classes, `aggregate_layout` lays out aggregates, and `hosts` fixes each host's registers, syscall numbers and import
//! libraries.

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod aggregate_layout;
mod boundary_application;
mod boundary_values;
mod callback_materializations;
mod fnv1a;
mod host_operations;
mod hosts;
mod plans;

pub use aggregate_layout::{
    ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout,
    evaluate_conventional_sum_layout,
};
pub use boundary_application::{
    BoundaryCallbackLayout, BoundaryCallingApplication, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterShape, BoundaryOpaqueApplication, BoundarySignatureGeometry,
    CallbackDataGeometry, CallbackInlineFieldGeometry, CallbackSlotGeometry,
};
pub use boundary_values::{
    BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape,
};
pub use callback_materializations::{
    BoundaryNativeParameterOrigin, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, LayoutPlanId, LayoutSlotId,
    NativeCallbackDemand, NativeParameterApplication, NativeParameterId, NativePlace,
    StaticMachineBinderId, callback_layout_field_slot_id, callback_layout_plan_id,
    callback_layout_slot_id, callback_nominal_report_fingerprint, callback_plan_laid_layout_id,
    callback_requirement_id, nominal_callback_native_parameter_id,
};
pub use host_operations::external_bindings::{ExternalBindingKind, ExternalBindingRow};
pub use host_operations::{HostCapability, HostOperation};
pub use hosts::darwin::{DARWIN_LIBOBJC_PATH, darwin_import_library};
pub use hosts::windows::windows_import_library;
pub use plans::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, IndirectPointerLocation, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, PlanDiagnostic, Preemption, RegisterSet,
    StateFootprintEvidence, StatePlan, SystemVEightbyteClass, ValidatedBoundaryEntryPlan,
    ValueClass, ValueLocation, ValuePlacement, ValueShape, byte_size_at_offset_zero,
    encode_state_plan_identity, evaluate_call_plan, evaluate_ordinary_boundary_entry_plan,
    gpr_code, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
