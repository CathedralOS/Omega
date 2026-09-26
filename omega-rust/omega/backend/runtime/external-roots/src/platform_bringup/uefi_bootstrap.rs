//! Returning-application UEFI system-table lifecycle composition.
//!
//! Header integrity is deliberately weaker than permission to use firmware
//! services. This module joins that target-owned integrity evidence to the
//! exact physical-arrival occurrence and a current Boot-Services-live phase
//! lease. The result remains a metadata-only lifecycle carrier: service-field
//! projection belongs to a later provider-specific edge.
//!
//! `firmware_ledger.rs` carries the application firmware ledger,
//! `system_table_lifecycle.rs` the lifecycle-scoped system table and its
//! join, `physical_arrival.rs` the physical arrival and its join,
//! `same_stack_budget.rs` the same-stack budget plan,
//! `adapter_composition.rs` the bootstrap adapter composition and
//! readiness and `tests.rs` the bootstrap tests.

mod adapter_composition;
mod exit_boot_services;
mod firmware_ledger;
mod get_memory_map;
mod handle_protocol_provider;
mod os_handoff;
mod os_handoff_cycle;
mod physical_arrival;
mod provider_projection;
mod same_stack_budget;
mod system_table_lifecycle;
#[cfg(test)]
mod tests;

pub use adapter_composition::{
    UefiApplicationBootstrapAdapterComposition, UefiApplicationBootstrapAdapterCompositionError,
    UefiApplicationBootstrapAdapterReadinessError, compose_uefi_application_bootstrap_adapter,
    prepare_uefi_application_bootstrap_adapter_invocation,
};
pub use exit_boot_services::{
    BoundUefiExitBootServicesInvocation, ExecutedUefiExitBootServicesInvocation,
    LifecycleScopedUefiExitBootServicesProvider, PlannedUefiExitBootServicesInvocation,
    UefiExitBootServicesAttemptError, UefiExitBootServicesAttemptOutcome,
    UefiExitBootServicesAttemptStatus, UefiExitBootServicesExecutionError,
    UefiExitBootServicesInvocationBindingError, UefiExitBootServicesInvocationPlanningError,
    UefiExitBootServicesProviderJoinError, UefiExitBootServicesProviderReleaseError,
    admit_uefi_exit_boot_services_execution, bind_uefi_exit_boot_services_invocation,
    execute_uefi_exit_boot_services, join_lifecycle_scoped_uefi_exit_boot_services_provider,
    prepare_uefi_exit_boot_services_invocation,
};
pub use firmware_ledger::UefiApplicationFirmwareLedger;
pub use get_memory_map::{
    BoundUefiGetMemoryMapInvocation, ExecutedUefiGetMemoryMapInvocation,
    LifecycleScopedUefiGetMemoryMapProvider, PlannedUefiGetMemoryMapInvocation,
    UefiGetMemoryMapAttemptError, UefiGetMemoryMapAttemptOutcome, UefiGetMemoryMapAttemptStatus,
    UefiGetMemoryMapExecutionError, UefiGetMemoryMapInvocationBindingError,
    UefiGetMemoryMapInvocationPlanningError, UefiMemoryMapAcquisition, UefiMemoryMapBuffer,
    admit_uefi_get_memory_map_execution, bind_uefi_get_memory_map_invocation,
    execute_uefi_get_memory_map, join_lifecycle_scoped_uefi_get_memory_map_provider,
    prepare_uefi_get_memory_map_invocation,
};
pub use handle_protocol_provider::{
    BoundUefiHandleProtocolInvocation, ExecutedUefiHandleProtocolInvocation,
    LifecycleScopedUefiHandleProtocolProvider, LifecycleScopedUefiLoadedImageCorrespondence,
    PlannedUefiHandleProtocolInvocation, UEFI_LOADED_IMAGE_PROTOCOL_GUID,
    UefiHandleProtocolExecutionError, UefiHandleProtocolExecutionStatus,
    UefiHandleProtocolInterfaceOutputSlot, UefiHandleProtocolInvocationBindingError,
    UefiHandleProtocolInvocationPlanningError, UefiHandleProtocolLoadedImageCallError,
    UefiHandleProtocolProviderJoinError, UefiHandleProtocolProviderReleaseError, UefiProtocolGuid,
    admit_uefi_loaded_image_handle_protocol_execution,
    bind_uefi_loaded_image_handle_protocol_invocation, execute_uefi_loaded_image_handle_protocol,
    join_lifecycle_scoped_uefi_handle_protocol_provider,
    prepare_uefi_loaded_image_handle_protocol_invocation,
};
pub use os_handoff::{
    UefiErrorStatus, UefiExitBootServicesProviderResult, UefiOsHandoffComplete,
    UefiOsHandoffExhausted, UefiOsHandoffLedger, UefiOsHandoffMapAcquired,
    UefiOsHandoffMapAcquisitionError, UefiOsHandoffMapRequired, UefiOsHandoffProgress,
    UefiOsHandoffTransitionError,
};
pub use os_handoff_cycle::{
    UefiOsHandoffCycleRejection, UefiOsHandoffCycleResolution, drive_uefi_os_handoff_cycle,
};
pub use physical_arrival::{
    UefiApplicationBootstrapAdapterInvocationReadiness, UefiApplicationPhysicalArrival,
    UefiApplicationPhysicalArrivalJoinError, join_uefi_application_physical_arrival,
};
pub use provider_projection::{
    LifecycleScopedUefiBootServicesProjection, UefiBootServicesProjectionError,
    UefiBootServicesProjectionReleaseError, project_uefi_application_boot_services,
};
pub use same_stack_budget::{
    UefiApplicationBootstrapSameStackBudgetPlan, UefiApplicationBootstrapSameStackDemandComponents,
    plan_uefi_application_bootstrap_same_stack_budget,
    plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter,
};
pub use system_table_lifecycle::{
    LifecycleScopedUefiSystemTable, ReleasedUefiSystemTableScope, UefiBootServicesPhaseLease,
    UefiImageHandleProvenance, UefiSystemTableLifecycleJoinError,
    UefiSystemTableOccurrenceProvenance, UefiSystemTableScopeReleaseError,
    join_lifecycle_scoped_uefi_system_table,
};
