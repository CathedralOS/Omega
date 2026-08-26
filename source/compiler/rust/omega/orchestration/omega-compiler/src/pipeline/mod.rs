mod adapter_dispatch;
mod artifacts;
mod boundary_report;
mod build_config;
mod build_replay_record;
mod build_staged_output;
mod calling_policy_plans;
mod checked_entry;
pub mod compile_options;
mod compile_policy;
pub mod compile_report;
pub mod compiler;
mod compiler_executable_commitment;
mod component_progress;
mod float_intrinsic_dispatch;
pub mod frontend;
mod operator_adapter_dispatch;
mod optimization_gate;
mod output;
mod package_compilation;
mod package_declaration_admission;
mod package_review;
mod package_source_consumption;
mod program_entry_physical;
mod program_entry_source_signature;
mod program_local_storage_custody;
mod program_storage_continuation_inbound;
mod program_storage_emitted_argument_binding;
mod program_storage_entry;
mod program_storage_extent_operand;
mod program_storage_extent_value;
mod program_storage_reserved_outgoing_frame;
mod program_storage_root_argument_binding;
mod program_storage_root_authority;
mod program_storage_source_call;
mod program_storage_wrapper;
mod program_storage_wrapper_arrival;
mod program_storage_wrapper_body;
mod program_storage_wrapper_evidence;
mod program_storage_wrapper_frame;
mod project;
mod provider_approval;
mod provider_plans;
pub mod source;
mod source_inspection;
mod source_profile;
mod stage;
mod stages;
mod target_machines;
mod task_plans;
mod terminal_component_candidate;
mod terminal_component_driver;
mod timing;
mod trust_lockfile;
mod trust_report;
mod wire_report;

pub use artifacts::{
    PROGRAM_STORAGE_INSTALLATION_ARTIFACT, program_storage_installation_record_json,
};
pub use build_config::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildEvaluationUsage, BuildFilesystemAuthorizedPath,
    BuildFilesystemByteOperand, BuildFilesystemGrantAccess, BuildFilesystemGrantRefusal,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleIdentity,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemLogicalHandleOutput,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservation,
    BuildFilesystemMetadataObservationKind, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemMutableI64Operand,
    BuildFilesystemMutableI64OperandResolution, BuildFilesystemObservedByteRegion,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationAttempt,
    BuildFilesystemOperationResult, BuildFilesystemPathLikeOperand, BuildFilesystemProvider,
    BuildFilesystemReturnedPath, BuildFilesystemReturnedPathCompleteness,
    BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildFilesystemScalarOperandValue, BuildObservationClass, BuildObservationSummary,
};
pub use build_replay_record::{
    BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits,
    ReviewOnlyBuildFilesystemReplayRecord, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
pub use build_staged_output::{
    BuildStagedOutputMaterializationError, BuildStagedOutputTree, BuildStagedOutputTreeCommitment,
};
pub use calling_policy_plans::evaluate_calling_policy_plan;
pub use checked_entry::{
    CheckedCompilation, compile_to_checked, compile_to_checked_with_packages,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir, compile_to_checked_with_replay_record,
};
pub use compile_options::{ArtifactEmissionPolicy, CompileOptions};
pub use compile_policy::ExecutableTcbBuildPolicy;
pub use compile_report::{
    CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, TerminalComponentDeploymentReportError,
};
pub use compiler::{CompileHarnessRequest, CompileRequest, compile, compile_harness};
pub use compiler_executable_commitment::{
    CompilerExecutableCommitment, CompilerExecutableCommitmentError,
};
pub use output::{
    OwnedTerminalComponentDeploymentError, SuppliedTerminalComponentDeploymentError,
    TerminalComponentDeploymentInputOwner, TerminalComponentDeploymentInputRejection,
    TerminalComponentDeploymentInputs, TerminalComponentDeploymentOutputError,
    TerminalComponentDeploymentOutputStage, TerminalComponentDeploymentSupply,
    acquire_and_deploy_terminal_component_output, deploy_and_write_terminal_component_output,
    deploy_supplied_terminal_component_output, write_finalized_terminal_component_output,
};
pub use package_compilation::{
    PackageCompilationInputError, PackageCompilationInputs, PackageDependencyBinding,
    PackageDependencyClosure, PackageSourceBinding,
};
pub use package_review::{
    CheckedPackageCallableReview, CheckedPackageProviderReview, CheckedPackageReviewProjection,
    DecodedPackageReviewCanonicalRow, OrdinaryPackageObligationLedger,
    OrdinaryPackageObligationLedgerRecoveryError, OrdinaryPackageObligationRow,
    PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION, PACKAGE_REVIEW_ENCODING_VERSION,
    PACKAGE_REVIEW_ROW_ENCODING_VERSION, PackageReviewArithmeticDomain,
    PackageReviewByteSequencePredicate, PackageReviewCallableConformance,
    PackageReviewCallableContract, PackageReviewCallableParameter, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
    PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource, PackageReviewCapabilityFlow,
    PackageReviewCastForm, PackageReviewCheckedServiceReach, PackageReviewConformanceBound,
    PackageReviewConformanceShape, PackageReviewConformanceSubject, PackageReviewConstShape,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewCrash, PackageReviewCrashCall,
    PackageReviewCrashInterface, PackageReviewCrashPredicate, PackageReviewCrashRoute,
    PackageReviewCrashRouteGuard, PackageReviewCrashSite, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthorityClass, PackageReviewDangerousAuthoritySlack,
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember, PackageReviewDataShape,
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape, PackageReviewEncodingError,
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewExternalBinding,
    PackageReviewExternalExecutableSupply, PackageReviewInstallationReach,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewMachineParameterValue, PackageReviewMutation, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewOperatorCoordinate, PackageReviewOperatorShape,
    PackageReviewPermissionClaim, PackageReviewPermissionSource, PackageReviewProgressPremise,
    PackageReviewProgressSubject, PackageReviewPropositionApplication,
    PackageReviewPropositionBinder, PackageReviewPropositionBinderArgument,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewRepresentationTcb, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSynchronousInvocation, PackageReviewSyntheticSourceKind, PackageReviewTermination,
    PackageReviewToolchainSourceIdentity, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
    ordinary_package_obligation_ledger_from_compiler_rows, project_checked_package_review,
    reconstruct_ordinary_package_obligation_ledger, recover_ordinary_package_obligation_ledger,
    validate_ordinary_package_obligation_ledger,
};
pub use package_source_consumption::PackageSourceConsumptionCommitment;
pub use program_entry_physical::ProgramEntryPhysicalContractPlan;
pub use program_entry_source_signature::{
    ProgramEntrySourceExtentFieldLayout, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceVisibleParameterSignature,
    SelectedProgramEntrySourceSignature,
};
pub use program_local_storage_custody::{
    ProgramLocalStorageCustody, ProgramLocalStorageCustodyError,
};
pub use program_storage_continuation_inbound::{
    ProgramStorageEntryContinuationInboundArgument, ProgramStorageEntryContinuationInboundPlan,
};
pub use program_storage_emitted_argument_binding::{
    ProgramStorageEntryEmittedWholeRootArgumentCarrier,
    ProgramStorageEntryEmittedWholeRootArgumentError,
    bind_program_local_storage_entry_emitted_whole_root_arguments,
    bind_program_storage_entry_emitted_whole_root_arguments,
};
pub use program_storage_entry::{
    InstalledImageSubextent, InstalledProgramStorageRoots, PartitionedProgramStorageRoots,
    ProgramEntryReceiverActivation, ProgramEntryReceiverActivationError,
    ProgramEntryReceiverPlacementRecord, ProgramEntryReceiverStoragePlan,
    ProgramLocalEntryReceiverActivation, ProgramLocalEntryReceiverActivationError,
    ProgramLocalStorageAccountHandoffError, ProgramLocalStorageInstallationHandoffError,
    ProgramLocalStorageRecordEmissionError, ProgramLocalStorageSubjectHandoffError,
    ProgramStorageEntryBridgeError, ProgramStorageEntryContinuationReceiverBindingError,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryExecutorDispatch,
    ProgramStorageEntryNativeBridgePlan, ProgramStorageEntryParameter,
    ProgramStorageEntryPlanBinding, ProgramStorageEntryProviderInvocation,
    ProgramStorageEntrySourceContinuationHandoff, ProgramStorageInstallationHandoffError,
    ProgramStorageInstallationRecord, ProgramStorageInstalledExtentRecord,
    ProgramStoragePartitionError, ProgramStorageRecordEmissionError, ProgramStorageRootInput,
    ProgramStorageRootInstallationError, RecordedProgramLocalStorageInstallation,
    RecordedProgramStorageInstallation, ReservedProgramEntryReceiverStorage,
    SelectedProgramStorageEntryPlan, bind_emitted_program_storage_entry_native_bridge,
    bind_generated_program_storage_entry_plan, bind_program_storage_entry_plan,
    establish_program_storage_entry_program_local_roots,
    install_and_activate_program_storage_entry_receiver,
    install_established_program_storage_entry_program_local_roots,
    install_program_storage_entry_provider_invocation,
};
pub use program_storage_extent_operand::{
    ProgramStorageEntryExtentOperandImage, ProgramStorageEntryWholeRootOperandCarrier,
    ProgramStorageEntryWholeRootOperandError, bind_program_local_storage_entry_whole_root_operands,
    bind_program_storage_entry_whole_root_operands,
};
pub use program_storage_extent_value::{
    ProgramStorageEntryExtentLogicalValue, ProgramStorageEntryWholeRootLogicalValueCarrier,
    ProgramStorageEntryWholeRootLogicalValueError,
    bind_program_local_storage_entry_whole_root_logical_values,
    bind_program_storage_entry_whole_root_logical_values,
};
pub use program_storage_reserved_outgoing_frame::{
    ProgramStorageEntryOutgoingStackWord, ProgramStorageEntryReservedOutgoingStackFrameError,
    ProgramStorageEntryReservedOutgoingStackFramePlan,
    reserve_program_local_storage_entry_outgoing_stack_frame,
    reserve_program_storage_entry_outgoing_stack_frame,
};
pub use program_storage_root_argument_binding::{
    ProgramLocalStorageRecordedWholeRootArgumentError,
    ProgramLocalStorageRecordedWholeRootArgumentRecovery,
    ProgramStorageEntryRecordedWholeRootArgumentError,
    ProgramStorageEntryRecordedWholeRootArgumentRecovery,
    ProgramStorageEntryWholeRootArgumentBinding, ProgramStorageEntryWholeRootArgumentCarrier,
    ProgramStorageEntryWholeRootArgumentError, bind_program_storage_entry_whole_root_arguments,
    bind_recorded_program_local_storage_entry_whole_root_arguments,
    bind_recorded_program_storage_entry_whole_root_arguments,
};
pub use program_storage_root_authority::{
    ProgramStorageEntryInitialStorageAuthorityKind, ProgramStorageEntryRootAuthorityDisposition,
    ProgramStorageEntryRootAuthorityDispositionError, ProgramStorageEntryWholeRootAuthority,
    ProgramStorageEntryWholeRootAuthorityError,
};
pub use program_storage_source_call::{
    ProgramStorageEntryContinuationAbiPlan, ProgramStorageEntryContinuationReceiverAbiPlan,
    ProgramStorageEntryContinuationReceiverBinding,
    ProgramStorageEntryContinuationVisibleArgumentPlan,
};
pub use program_storage_wrapper::{
    ProgramStorageEntryRootRole, ProgramStorageEntryWrapperReceiverTransfer,
    ProgramStorageEntryWrapperRootTransferPlan, ProgramStorageEntryWrapperTransferPlan,
};
pub use program_storage_wrapper_arrival::{
    ProgramStorageEntryEmittedArrivalCopyEvidence, ProgramStorageEntryEmittedArrivalEvidence,
    ProgramStorageEntryEmittedArrivalRootEvidence,
};
pub use program_storage_wrapper_body::{
    ProgramStorageEntryWrapperBodyTemplatePlan, ProgramStorageEntryWrapperBodyTemplateStep,
};
pub use program_storage_wrapper_evidence::ProgramStorageEntryEmittedWrapperEvidence;
pub use program_storage_wrapper_frame::{
    ProgramStorageEntryWrapperCallerFrameError, ProgramStorageEntryWrapperCallerFramePlan,
    ProgramStorageEntryWrapperCallerFrameStep,
    plan_program_local_storage_entry_wrapper_caller_frame,
    plan_program_storage_entry_wrapper_caller_frame,
};
pub use provider_plans::{
    AdmittedExternalRootEntryFactHandoff, BoundExternalRootPostHandoffWriterInvocation,
    ExternalRootPostHandoffWriterBindingError, SelectedExternalRootEntryFactBinding,
    SelectedExternalRootPostHandoffWriterPreparation, SelectedExternalRootProviderPlan,
    SelectedExternalRootWriterPreparationError,
    ValidatedWrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootConsumerValidationError,
    WrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootWriterRecoveryError, bind_external_root_post_handoff_writer_invocation,
    compiler_intrinsic_diagnostic_label, selected_external_root_entry_fact_bindings,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};
pub use psi_access_plans::{ValidatedAccessPlan, ValidatedPlacementPlan};
pub use psi_build_time_evaluation::{
    BuildTimeValue, compute_access_plan, compute_layout_plan, compute_placement_plan,
    evaluate_and_materialize_typed_owned_layout_into, materialize_typed_owned_layout_into,
};
pub use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConsumptionInstant, DataSymbolId,
    EntryStubId, MaterializationAction, MaterializationContext, MaterializationDiagnostic,
    MaterializationWrite, RelocationTarget, ScalarFieldSchema, ScalarFieldValue,
    SymbolicFieldValue, SymbolicMaterializationPlan, decode_scalar_layout,
    derive_symbolic_materialization, materialize_aggregate_layout_into,
    materialize_scalar_layout_into, normalized_layout_plan_fingerprint,
};
pub use psi_layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
pub use source_inspection::{
    SOURCE_CLOSURE_SNAPSHOT_SCHEMA, SourceClosureSnapshot, SourceClosureSnapshotEntry,
    inspect_source_closure,
};
pub use source_profile::{
    SOURCE_FEATURE_CATALOG, SOURCE_FEATURE_CENSUS_SCHEMA, SOURCE_FEATURE_IDS, SOURCE_RESOURCE_IDS,
    SourceFeatureCensus, SourceFeatureCount, SourceResourceObservation, census_source_closure,
};
pub use terminal_component_candidate::{
    TerminalComponentCandidate, TerminalComponentCandidateParts,
    TerminalComponentProviderExecution, TerminalComponentProviderSettlement,
    stage_terminal_component,
};
pub use terminal_component_driver::{
    TerminalComponentDriverError, TerminalComponentStagingInputs,
    stage_acquire_and_deploy_terminal_component_output,
};
