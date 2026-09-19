//! Normalized ledger for entry points invoked from outside Omega's call graph.
//!
//! Installing code does not make any of its entries analysis roots. A slot
//! owner separately installs one admitted entry under a validated boundary
//! plan. That operation records the root's effects, trust receipts, stack and
//! nesting policy, WCSU demand, and component/version pins. The returned
//! handle borrows the installed code, preventing retirement while the root is
//! reachable.
//!
//! `installed_root_ledger.rs` is the root: the ledger, its records and the
//! install, remove and teardown operations. `root_admission.rs` is how one
//! validated root enters a slot, `interrupt_entries.rs` and
//! `interrupt_masks.rs` carry one interrupt entry from arrival to settled
//! exit, `identities.rs` holds every normalized coordinate and
//! `diagnostic.rs` the shared failure type. The other modules each own one
//! evidence family: fuel, stack demand, program-local roots and extents,
//! provider execution, progress profiles, interrupt tables, secondary
//! processors and UEFI bootstrap.
//!
//! The evidence families are grouped into folders: `root_entry/`,
//! `interrupts/`, `stack_and_fuel/`, `program_local/` and
//! `platform_bringup/`, each opened by a route file that lists its modules.

mod diagnostic;
mod identities;
mod installed_root_ledger;
mod interrupts;
mod platform_bringup;
mod program_local;
mod root_entry;
mod stack_and_fuel;
#[cfg(test)]
mod tests;

pub use diagnostic::ExternalRootDiagnostic;
pub use executable_installation::{ArtifactId, InstallationScopeId, InstalledCodeId};
pub use identities::{
    AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPinId, ExternalRootId, FuelProvisionId, FuelValidationReceiptId,
    GatewayAdmissionReceiptId, GatewayDispatchContractId, InstalledProviderOccurrenceId,
    InterruptAcknowledgementId, InterruptAcknowledgementReceiptId, InterruptEntryReceiptId,
    InterruptInvocationId, InterruptMaskControlId, InterruptMaskGuardId, InterruptMaskStateId,
    InterruptMaskTransitionReceiptId, InterruptTableEstablishmentId, InterruptTableProfileId,
    InterruptTablePublicationAuthorityId, InterruptTablePublicationId,
    InterruptTablePublicationReceiptId, NestingRelationId, OpaqueCallbackProviderId,
    OpaqueCallbackRegistrationCapacityOccurrenceId, OpaqueCallbackRegistrationId,
    OpaqueCallbackRegistrationReceiptId, OpaqueCallbackUnregistrationContractId,
    OpaqueCallbackUnregistrationReceiptId, ProcessLifetimeGatewayId,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId, ProviderExecutionId,
    ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
    ProviderOccurrenceInstallationReceiptId, ProviderPlanId, RootAdmissionId, RootEffectId,
    RootProviderId, RootRemovalReceiptId, RootSlotId, RootSlotOwnerId,
    SecondaryProcessorOccurrenceId, SecondaryProcessorQuiescenceReceiptId,
    SecondaryProcessorStartupInvocationId, SecondaryProcessorStartupProfileId,
    SecondaryProcessorStartupReceiptId, StackValidationReceiptId, StateValidationReceiptId,
    TrustReceiptId, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiBootServicesTableOccurrenceId, UefiExitBootServicesReceiptId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiMemoryMapKeyId, UefiMemoryMapSnapshotId,
    UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
    UefiOsHandoffStackEvidenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
    X86_64GateProfileValidationReceiptId,
};
pub use installation_evidence::{ObjectEvidence, StackDemandEvidence};
pub use installed_root_ledger::{
    InstalledExternalRoot, InstalledRootLedger, InstalledRootRecord, InstalledRootRemoval,
    InstalledRootTeardownError, RootInstallError, RootRemovalError, RootRemovalReceipt,
};
pub use interrupts::interrupt_entries::{
    CompletedInterruptAcknowledgement, CompletedInterruptEntry, InstalledInterruptCompletionRoute,
    InterruptAcknowledgement, InterruptAcknowledgementError, InterruptAcknowledgementReceipt,
    InterruptEntryFinishError, InterruptEntryObligations, InterruptEntryReceipt,
    InterruptEntryStartError, InterruptEpochTurnError, InterruptEpochTurnReport,
    InterruptPreemptionReport, PendingInterruptExit,
};
pub use interrupts::interrupt_masks::{
    InterruptMaskControl, InterruptMaskGuard, InterruptMaskRestoreError,
    InterruptMaskRestoreReceipt, InterruptMaskSaveError, InterruptMaskSaveReceipt,
};
pub use interrupts::interrupt_table::{
    EstablishedInterruptTable, ExecutedInterruptTablePublication,
    INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES, InterruptDescriptorTableState,
    InterruptTableAdmissionError, InterruptTableClose, InterruptTableCompletionError,
    InterruptTableDescriptorOperand, InterruptTableEstablishedMember, InterruptTableGateDescriptor,
    InterruptTableLedger, InterruptTableMember, InterruptTableMemberPlan, InterruptTableObligation,
    InterruptTableProfile, InterruptTableProviderError, InterruptTablePublication,
    InterruptTablePublicationAuthority, InterruptTablePublicationError,
    InterruptTablePublicationOutcome, InterruptTablePublicationReceipt,
    InterruptTablePublicationRefusal, InterruptTablePublicationScope, PublishedInterruptTable,
    X86_64_GATE_DESCRIPTOR_BYTES, X86_64_IST_SLOT_LIMIT,
};
pub use platform_bringup::secondary_processor::{
    InstalledSecondaryProcessorTrampoline, SecondaryProcessorAccount,
    SecondaryProcessorAccountError, SecondaryProcessorAdmissionError, SecondaryProcessorBindError,
    SecondaryProcessorCompletionError, SecondaryProcessorQuiescenceOutcome,
    SecondaryProcessorQuiescenceReceipt, SecondaryProcessorQuiescenceRefusal,
    SecondaryProcessorRecord, SecondaryProcessorRegimeTransition, SecondaryProcessorRetirement,
    SecondaryProcessorRetirementError, SecondaryProcessorStartError, SecondaryProcessorStarted,
    SecondaryProcessorStartupInvocation, SecondaryProcessorStartupLedger,
    SecondaryProcessorStartupOutcome, SecondaryProcessorStartupProfile,
    SecondaryProcessorStartupReceipt, SecondaryProcessorStartupRefusal,
    SecondaryProcessorStartupUnconfirmed, SecondaryProcessorStartupVerdict,
    SecondaryProcessorWithdrawError, SecondaryProcessorWithdrawal,
    bind_secondary_processor_trampoline,
};
pub use platform_bringup::uefi_bootstrap::{
    BoundUefiExitBootServicesInvocation, BoundUefiGetMemoryMapInvocation,
    BoundUefiHandleProtocolInvocation, ExecutedUefiExitBootServicesInvocation,
    ExecutedUefiGetMemoryMapInvocation, ExecutedUefiHandleProtocolInvocation,
    LifecycleScopedUefiBootServicesProjection, LifecycleScopedUefiExitBootServicesProvider,
    LifecycleScopedUefiGetMemoryMapProvider, LifecycleScopedUefiHandleProtocolProvider,
    LifecycleScopedUefiLoadedImageCorrespondence, LifecycleScopedUefiSystemTable,
    PlannedUefiExitBootServicesInvocation, PlannedUefiGetMemoryMapInvocation,
    PlannedUefiHandleProtocolInvocation, ReleasedUefiSystemTableScope,
    UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiApplicationBootstrapAdapterComposition,
    UefiApplicationBootstrapAdapterCompositionError,
    UefiApplicationBootstrapAdapterInvocationReadiness,
    UefiApplicationBootstrapAdapterReadinessError, UefiApplicationBootstrapSameStackBudgetPlan,
    UefiApplicationBootstrapSameStackDemandComponents, UefiApplicationFirmwareLedger,
    UefiApplicationPhysicalArrival, UefiApplicationPhysicalArrivalJoinError,
    UefiBootServicesPhaseLease, UefiBootServicesProjectionError,
    UefiBootServicesProjectionReleaseError, UefiErrorStatus, UefiExitBootServicesAttemptError,
    UefiExitBootServicesAttemptOutcome, UefiExitBootServicesAttemptStatus,
    UefiExitBootServicesExecutionError, UefiExitBootServicesInvocationBindingError,
    UefiExitBootServicesInvocationPlanningError, UefiExitBootServicesProviderJoinError,
    UefiExitBootServicesProviderReleaseError, UefiExitBootServicesProviderResult,
    UefiGetMemoryMapAttemptError, UefiGetMemoryMapAttemptOutcome, UefiGetMemoryMapAttemptStatus,
    UefiGetMemoryMapExecutionError, UefiGetMemoryMapInvocationBindingError,
    UefiGetMemoryMapInvocationPlanningError, UefiHandleProtocolExecutionError,
    UefiHandleProtocolExecutionStatus, UefiHandleProtocolInterfaceOutputSlot,
    UefiHandleProtocolInvocationBindingError, UefiHandleProtocolInvocationPlanningError,
    UefiHandleProtocolLoadedImageCallError, UefiHandleProtocolProviderJoinError,
    UefiHandleProtocolProviderReleaseError, UefiImageHandleProvenance, UefiMemoryMapAcquisition,
    UefiMemoryMapBuffer, UefiOsHandoffComplete, UefiOsHandoffCycleRejection,
    UefiOsHandoffCycleResolution, UefiOsHandoffExhausted, UefiOsHandoffLedger,
    UefiOsHandoffMapAcquired, UefiOsHandoffMapAcquisitionError, UefiOsHandoffMapRequired,
    UefiOsHandoffProgress, UefiOsHandoffTransitionError, UefiProtocolGuid,
    UefiSystemTableLifecycleJoinError, UefiSystemTableOccurrenceProvenance,
    UefiSystemTableScopeReleaseError, admit_uefi_exit_boot_services_execution,
    admit_uefi_get_memory_map_execution, admit_uefi_loaded_image_handle_protocol_execution,
    bind_uefi_exit_boot_services_invocation, bind_uefi_get_memory_map_invocation,
    bind_uefi_loaded_image_handle_protocol_invocation, compose_uefi_application_bootstrap_adapter,
    drive_uefi_os_handoff_cycle, execute_uefi_exit_boot_services, execute_uefi_get_memory_map,
    execute_uefi_loaded_image_handle_protocol,
    join_lifecycle_scoped_uefi_exit_boot_services_provider,
    join_lifecycle_scoped_uefi_get_memory_map_provider,
    join_lifecycle_scoped_uefi_handle_protocol_provider, join_lifecycle_scoped_uefi_system_table,
    join_uefi_application_physical_arrival, plan_uefi_application_bootstrap_same_stack_budget,
    plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter,
    prepare_uefi_application_bootstrap_adapter_invocation,
    prepare_uefi_exit_boot_services_invocation, prepare_uefi_get_memory_map_invocation,
    prepare_uefi_loaded_image_handle_protocol_invocation, project_uefi_application_boot_services,
};
pub use program_local::program_local_extents::{
    ProgramLocalExtentAggregateRetirementError, ProgramLocalExtentMaterializationError,
    ProgramLocalExtentRegistry, ProgramLocalExtentRetirementError, ReleasedRetainedForeignArgument,
    RetainedForeignAccess, RetainedForeignArgument, RetainedForeignArgumentDisposition,
    RetainedForeignArgumentError, RetainedForeignArgumentId, RetainedForeignArgumentRequest,
    RetiredProgramLocalExtent,
};
pub use program_local::program_local_roots::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity,
    InstalledProgramLocalRootEpochCohort, InstalledProgramLocalRootEpochCohortId,
    InstalledProgramLocalRootOccurrence, InstalledProgramLocalRootOccurrenceId,
    InstalledProgramLocalRootSubject, ProgramLocalEntryActivation,
    ProgramLocalEntryActivationLeaveError, ProgramLocalRootBatchEstablishmentError,
    ProgramLocalRootCoexistenceReport, ProgramLocalRootCohortMember,
    ProgramLocalRootCohortSealError, ProgramLocalRootEntryInvocationId,
    ProgramLocalRootEpochAggregate, ProgramLocalRootEpochAggregateCapacity,
    ProgramLocalRootEpochAggregateSnapshot, ProgramLocalRootEpochRuntime,
    ProgramLocalRootEstablishmentError, ProgramLocalRootInstallationLedger,
    ProgramLocalRootInstalledPrebinding, ProgramLocalRootInstalledPrebindingCount,
    ProgramLocalRootLineageId, ProgramLocalRootOccurrenceRetirementError,
    ProgramLocalRootPrebindingId, ProgramLocalRootRetirementError, ProgramLocalRootScalarBinding,
    ProgramLocalRootScalarSource, ProgramLocalRootSchemaDigest, ProgramLocalRootSubjectPlaceId,
    RetiredProgramLocalRootOccurrence, compose_program_local_root_coexistence_report,
};
pub use root_entry::opaque_callback_replacement::{
    CompletedOpaqueCallbackUnregistration, OpaqueCallbackRegistrationCapacityOccurrence,
    OpaqueCallbackRegistrationError, OpaqueCallbackRegistrationReceipt,
    OpaqueCallbackUnregistrationError, OpaqueCallbackUnregistrationReceipt,
    ProcessLifetimeGatewayAdmissionError, ProcessLifetimeGatewayAdmissionReceipt,
    ProcessLifetimeOpaqueCallback, ReclaimableOpaqueCallback,
    admit_process_lifetime_opaque_callback, admit_reclaimable_opaque_callback,
};
pub use root_entry::progress_profile_installation::{
    AdmittedProgressProfileEstablishment, ComponentProgressDemandIdentity,
    ComponentProgressReceiptBinding, ComponentProgressSealError, InstalledComponentProgressClosure,
    InstalledProviderOccurrence, InstalledProviderOccurrenceClosure,
    ProgressProfileEstablishmentAdmissionError, ProgressProfileEstablishmentAttestation,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrencePlanBinding,
};
pub use root_entry::provider_execution::{
    AdmittedProviderExecution, OpaqueProviderExitAssurance,
    PreparedExternalRootPostHandoffWriterInvocation, PreparedExternalRootWriterExecutionError,
    ProviderExecution, ValidatedWrittenExternalRootPostHandoffWriterDestination,
    WrittenExternalRootConsumerValidationError, WrittenExternalRootPostHandoffWriterDestination,
    WrittenExternalRootWriterRecoveryError,
};
pub use root_entry::required_root_slots::{
    InstalledRequiredRootSlot, InstalledRequiredRootSlotClosure, TargetRequiredRootSlotSelection,
    VerifiedRequiredRootSlot, VerifiedRequiredRootSlotClosure,
    verify_target_required_root_slot_closure,
};
pub use root_entry::root_admission::{
    AdmittedEntryQualification, AdmittedEntrySubject, AdmittedResultQualification,
    AdmittedResultSubject, RootAdmission, RootSlotAuthority,
};
pub use root_entry::root_validation::{
    ComponentVersionPin, ExternalRootCandidate, ExternalRootEntryClaim, ExternalRootResultClaim,
    MachineStateResourceColumn, ResolvedRootServiceReach, ValidatedExternalRoot,
    validate_external_root,
};
pub use semantic_vocabulary::FuelScheduleIdentity;
pub use stack_and_fuel::epoch_stack_demand::{
    AdapterStackRealizationOrigin, AdmittedOpaqueArrivalContextSet, ArrivalStackRealizationOrigin,
    BoundEpochStackComposition, BoundEpochStackCompositionInput, ComposedEpochStackDemand,
    DomainStackDemand, EntryStackRealizationEvidence, EpochStackComposition,
    EpochStackCompositionInput, GeneratedProgramStorageAdapterLiveFrameDemand,
    GeneratedProgramStorageAdapterStackEvidence, InstalledX86_64TargetDerivedHardwareArrival,
    ValidatedX86_64InstalledGateProfileRoster, X86_64GeneratedProgramStorageAdapterEmission,
    admit_opaque_arrival_context_set, bind_direct_generated_entry_stack_realization,
    bind_opaque_adapter_stack_realization,
    bind_x86_64_generated_program_storage_adapter_stack_realization,
    bind_x86_64_target_direct_entry_stack_realization, compose_bound_entry_stack_epochs,
    compose_entry_stack_epochs, derive_generated_program_storage_adapter_live_frame_demand,
    produce_x86_64_installed_hardware_entry_facts, validate_x86_64_installed_gate_profile_roster,
};
pub use stack_and_fuel::fixed_fuel::{
    ComposedFuelDemand, FixedFuelCall, FixedFuelLocalEvidence, FixedFuelProviderSummary,
    InstalledEntryFuelCertificate, InstalledSegmentFuelCatalog, InstalledSegmentFuelCertificate,
    LogicalFuelResourceColumn, bind_installed_entry_fuel, bind_installed_segment_fuel,
    bind_installed_segment_fuel_catalog, compose_fixed_fuel, validate_installed_entry_fuel,
    validate_installed_segment_fuel, validate_installed_segment_fuel_catalog,
};
pub use stack_and_fuel::stack_demand::{
    ArtifactStackComposition, ComposedStackDemand, InstalledEntryStackDemand, ProviderStackSummary,
    StackDomain, StackLocalEvidence, StackNestingEdge, StackNestingRelation, StackResourceColumn,
    bind_installed_entry_stack, compose_artifact_stacks, validate_installed_entry_stack,
};
