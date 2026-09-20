//! Calling conventions: value passing, stack realization, and callbacks per host.
//!
//! `plans` owns the calling-plan vocabulary the README's contract publishes;
//! `darwin`, `linux`, and `windows` fix each host's registers, syscall numbers,
//! and import libraries; `aggregate_layout` and `stack_realizations` decide how
//! aggregates and stack slots are passed; `callback_materializations` binds
//! native callbacks. Layout and selection consume these; they never re-derive them.
//!
//! Start at `host_operations.rs`: the host operation catalog, its
//! capabilities and the external binding rows that bind an operation to a
//! foreign symbol; `plans` owns the calling-plan vocabulary, `stack_realizations`
//! and `callback_materializations` realize stacks and callbacks,
//! `aggregate_layout` lays out aggregates, and `hosts` fixes each host's registers, syscall numbers and import
//! libraries.

mod aggregate_layout;
mod callback_materializations;
mod host_operations;
mod hosts;
mod plans;
mod stack_realizations;

pub use aggregate_layout::{
    AggregateLayoutError, ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout,
    evaluate_conventional_sum_layout,
};
pub use callback_materializations::{
    CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
    CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativeCallbackDemand,
    NativeParameterApplication, NativeParameterId, NativePlace, StaticMachineBinderId,
    callback_layout_field_slot_id, callback_layout_plan_id, callback_layout_slot_id,
    callback_native_parameter_id, callback_plan_laid_layout_id, callback_requirement_id,
    nominal_callback_native_parameter_id,
};
pub use host_operations::external_bindings::{ExternalBindingKind, ExternalBindingRow};
pub use host_operations::{HostCapability, HostOperation, HostOperationKey};
pub use hosts::darwin::{
    DARWIN_COREGRAPHICS_PATH, DARWIN_LIBOBJC_PATH, DARWIN_LIBSYSTEM_PATH, darwin_import_library,
};
pub use hosts::linux::{linux_clock_gettime_syscall_number, linux_nanosleep_syscall_number};
pub use hosts::windows::windows_import_library;
pub use plans::{
    BoundaryEntryPlan, BoundaryPlanDiagnostic, BoundaryPlanResult, CallPlan, CallSignature,
    CallingPolicy, CallingPolicyRejection, ConcreteVariadicCallSignature, EntryControl, EntryStack,
    IndirectPointerLocation, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    PlanDiagnostic, Preemption, ProviderExitRealization, RegisterSet, StateFootprintEvidence,
    StatePlan, SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass, ValueLocation,
    ValuePlacement, ValueShape, compose_state_footprints, evaluate_call_plan,
    evaluate_darwin_aapcs64_variadic_boundary_entry_plan,
    evaluate_darwin_aapcs64_variadic_call_plan, evaluate_freestanding_program_entry_plan,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
    validate_call_plan, validate_call_return_mechanics_footprint,
    validate_composed_state_footprint, validate_outbound_call_footprint,
    validate_provider_exit_realization, validate_runtime_value_guard_footprint,
    validate_state_footprint,
};
pub use stack_realizations::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, InstalledEntryFactIdentity, StackDomainRef,
    StackOccupancy, ValidatedEntryStackDomainClosure, ValidatedEntryStackRealization,
    ValidatedX86_64DeriverStub, ValidatedX86_64InstalledHardwareEntryFacts,
    X86_64ArrivalMechanism, X86_64DeriverStub, X86_64DeriverStubContext,
    X86_64ErrorCodeDisposition, X86_64GateKind, X86_64HardwareStackSelection,
    X86_64InstalledArrivalContext, X86_64InstalledGateArrival, X86_64InstalledGateRealization,
    X86_64InstalledGateTssRealization, X86_64InstalledHardwareEntryFacts,
    X86_64InstalledInterruptStack, X86_64InstalledPrivilegeStack,
    X86_64InstalledTaskStateSegmentRealization, X86_64TargetDerivedHardwareArrival,
    X86_64TargetProfileIdentity, derive_x86_64_entry_exit_stub, derive_x86_64_hardware_arrival,
    validate_entry_stack_domain_closure, validate_entry_stack_realization,
    validate_x86_64_installed_hardware_entry_facts,
};
