#[path = "dispatch/adapter.rs"]
mod adapter_dispatch;
mod artifacts;
mod boundary_report;
#[path = "build/config.rs"]
mod build_config;
#[path = "build/replay_record.rs"]
mod build_replay_record;
#[path = "build/staged_output.rs"]
mod build_staged_output;
#[path = "provider/calling_policy_plans.rs"]
mod calling_policy_plans;
mod checked_entry;
mod compile_policy;
mod compiler_executable_commitment;
#[path = "provider/component_progress.rs"]
mod component_progress;
mod emitted_program;
#[path = "dispatch/float_intrinsic.rs"]
mod float_intrinsic_dispatch;
pub mod frontend;
pub(crate) mod legacy_driver;
#[path = "dispatch/operator_adapter.rs"]
mod operator_adapter_dispatch;
mod optimization_gate;
mod output;
#[path = "package/compilation.rs"]
mod package_compilation;
#[path = "package/declaration_admission.rs"]
mod package_declaration_admission;
#[path = "package/review.rs"]
mod package_review;
#[path = "package/source_consumption.rs"]
mod package_source_consumption;
#[path = "program_storage/entry_physical.rs"]
mod program_entry_physical;
#[path = "program_storage/entry_source_signature.rs"]
mod program_entry_source_signature;
#[path = "program_storage/local_storage_custody.rs"]
mod program_local_storage_custody;
#[path = "program_storage/continuation_inbound.rs"]
mod program_storage_continuation_inbound;
#[path = "program_storage/emitted_argument_binding.rs"]
mod program_storage_emitted_argument_binding;
#[path = "program_storage/entry.rs"]
mod program_storage_entry;
#[path = "program_storage/extent_operand.rs"]
mod program_storage_extent_operand;
#[path = "program_storage/extent_value.rs"]
mod program_storage_extent_value;
#[path = "program_storage/reserved_outgoing_frame.rs"]
mod program_storage_reserved_outgoing_frame;
#[path = "program_storage/root_argument_binding.rs"]
mod program_storage_root_argument_binding;
#[path = "program_storage/root_authority.rs"]
mod program_storage_root_authority;
#[path = "program_storage/source_call.rs"]
mod program_storage_source_call;
#[path = "program_storage/wrapper.rs"]
mod program_storage_wrapper;
#[path = "program_storage/wrapper_arrival.rs"]
mod program_storage_wrapper_arrival;
#[path = "program_storage/wrapper_body.rs"]
mod program_storage_wrapper_body;
#[path = "program_storage/wrapper_evidence.rs"]
mod program_storage_wrapper_evidence;
#[path = "program_storage/wrapper_frame.rs"]
mod program_storage_wrapper_frame;
mod project;
#[path = "provider/approval.rs"]
mod provider_approval;
#[path = "provider/plans.rs"]
mod provider_plans;
pub mod source;
mod source_inspection;
mod stage;
mod stages;
#[path = "provider/target_machines.rs"]
mod target_machines;
#[path = "provider/task_plans.rs"]
mod task_plans;
#[path = "terminal/compile_driver.rs"]
mod terminal_compile_driver;
#[path = "terminal/component_candidate.rs"]
mod terminal_component_candidate;
#[path = "terminal/component_driver.rs"]
mod terminal_component_driver;
#[path = "terminal/native_artifact.rs"]
mod terminal_native_artifact;
mod timing;
#[path = "trust/lockfile.rs"]
mod trust_lockfile;
#[path = "trust/report.rs"]
mod trust_report;
mod wire_report;

pub(crate) use crate::compiler::{
    CompileOptions, CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt,
};

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
    PackageGeneratedSource,
};
pub use calling_policy_plans::evaluate_calling_policy_plan;
pub(crate) use checked_entry::compile_to_checked_for_terminal;
pub use checked_entry::{
    CheckedCompilation, compile_to_checked, compile_to_checked_with_packages,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir, compile_to_checked_with_replay_record,
};
pub use compile_policy::ExecutableTcbBuildPolicy;
pub use compiler_executable_commitment::{
    CompilerExecutableCommitment, CompilerExecutableCommitmentError,
};
pub use omega_source_profile::{
    SOURCE_FEATURE_CATALOG, SOURCE_FEATURE_CENSUS_SCHEMA, SOURCE_FEATURE_IDS, SOURCE_RESOURCE_IDS,
    SourceFeatureCensus, SourceFeatureCount, SourceResourceObservation, census_source_closure,
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
    PackageDependencyClosure, PackageGeneratedSourceBundle, PackageSourceBinding,
};
pub use package_review::{
    CheckedPackageCallableReview, CheckedPackageProviderReview, CheckedPackageReviewProjection,
    DecodedPackageReviewCanonicalRow, ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION,
    ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION, OrdinaryPackageObligationLedger,
    OrdinaryPackageObligationLedgerFingerprint, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
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
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewInstallationReach, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewMachineParameterValue,
    PackageReviewMutation, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewOperatorCoordinate, PackageReviewOperatorShape, PackageReviewPermissionClaim,
    PackageReviewPermissionSource, PackageReviewProgressPremise, PackageReviewProgressSubject,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
    PackageReviewPropositionParameterApplication, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewPropositionShape,
    PackageReviewPublicPropositionBody, PackageReviewRepresentationAbiCommitment,
    PackageReviewRepresentationMechanism, PackageReviewRepresentationTcb,
    PackageReviewSemanticDependency, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind, PackageReviewSourceLocation,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSynchronousInvocation, PackageReviewSyntheticSourceKind, PackageReviewTermination,
    PackageReviewToolchainSourceIdentity, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
    decode_ordinary_package_obligation_ledger, decode_package_review_canonical_row,
    decode_package_review_canonical_row_with_limits, encode_ordinary_package_obligation_ledger,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
    ordinary_package_obligation_ledger_fingerprint,
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
    BuildTimeValue, ValidatedConstMaterialization, compute_access_plan, compute_layout_plan,
    compute_placement_plan, evaluate_and_materialize_typed_owned_layout_into,
    materialize_typed_owned_layout_into, validate_const_materializable_typed_owned_layout,
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
    SourceInspectionRoot, inspect_source_closure, inspect_source_closure_with_packages,
};
pub use terminal_compile_driver::{
    TerminalComponentCompileError, TerminalComponentCompileRequest,
    compile_terminal_component_output,
};
pub use terminal_component_candidate::{
    TerminalComponentCandidate, TerminalComponentCandidateParts,
    TerminalComponentProviderExecution, TerminalComponentProviderSettlement,
    stage_terminal_component,
};
pub use terminal_component_driver::{
    TerminalComponentDriverError, TerminalComponentStagingInputBindingError,
    TerminalComponentStagingInputs, stage_acquire_and_deploy_terminal_component_output,
};
pub use terminal_native_artifact::{
    TerminalNativeArtifact, TerminalNativeArtifactParts, TerminalNativeProviderExecution,
    TerminalNativeProviderSettlement, realize_terminal_native_artifact,
};
