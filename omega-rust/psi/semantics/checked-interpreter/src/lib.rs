//! Psi-owned checked/typed-tree interpreter used for build-time evaluation and
//! as a differential oracle while the terminal producer grows.
//! Start at [`interpreter`] for worker setup, authority selection, and result handling.
//!
//! Canonical portable execution belongs to the Psi-owned
//! `terminal-interpreter` crate. This crate retains the source-shaped
//! evaluator for constructs not yet represented in terminal Psi.
//!
//! The interpreter evaluates the program at the level of the typed/checked trees
//! (`checked_trees::CheckedTrees`, which derefs to `typed_trees::TypedTrees`)
//! -- above all backend lowering. It is therefore independent of the backend bugs
//! it must catch: if [`interpret_entry`] and the native binary disagree on exit
//! code or stdout for the same checked program, the backend is wrong.
//!
//! ## Value & store model (the crux: aliasing)
//! Every storage place -- local, struct field, machine instance -- is an
//! allocation-owned [`value::Cell`]. A `&mut place` argument evaluates to a
//! [`Value::Ref`] holding a clone of the *same* cell, so a write through the reference
//! mutates the original cell. Multi-level `&mut` aliasing is therefore correct by
//! construction -- this is exactly the property the native backend is known to get
//! wrong (an `&mut`-write through a call chain that does not persist). Once the
//! interpreter's coverage reaches such a program, an `interpret_entry != native`
//! mismatch localizes the bug instantly.
//!
//! ## Execution model
//! [`interpret_entry`] runs the exact machine selected by its caller. The
//! interpreter neither chooses a conventional entry spelling nor searches alternate
//! names. A machine instance is a [`Value::Struct`] with default-initialized
//! fields. A state has parameters + a sequence of statements + guarded transitions; the
//! first transition whose guard holds determines the next state (or the returned value /
//! terminal). Host-boundary calls (`exit_process`, `write`, `write_line`) on a
//! `boundary trait` machine drive exit code / stdout.
//!
//! ## Scope
//! Supported: multiple machines with per-instance contained sub-objects; symbol/group-based
//! machine + sibling-state resolution; self-field assignment; `let` locals; Integer / Bool /
//! Float / Binary (arith + compare + logical) / Unary / Name / Member / Indexed / Cast /
//! ArrayLiteral / StructLiteral expressions; fixed arrays and `.as_slice()`/`.as_mut_slice()`
//! slice views (a slice shares the array's element cells, preserving `&mut` aliasing);
//! width/signedness-aware `as` casts (int<->float, integer narrow/widen); multi-arm
//! value/guard transitions (subject, tuple, and boolean forms); value-calls returning a
//! scalar/struct; method calls on `&mut Data` reference params; `&mut`-aliased argument
//! passing, including MULTI-HOP forwarding (a `&mut` param passed onward as a bare name --
//! to a nested call or a transition-target state -- stays aliased, hop after hop);
//! `dyn Trait` dispatch by the receiver's RUNTIME type (works for any number of
//! impls -- AHEAD of the native backend, which only devirtualizes single-impl traits); the
//! transition guard SUBJECTS evaluate exactly once per transition evaluation (the
//! parser copies the subject call into every arm's guard; the per-frame memo reuses
//! the first arm's result instead of re-running the callee's side effects, matching
//! the native lowering's shared branch prelude); the
//! entry machine's value as the exit code; the Console boundary `exit_process`,
//! `write`/`write_line`, `write_error`/`write_error_line` (collected on a separate
//! stderr stream), and `read_line` (consuming `stdin`), including the imported std
//! `console`. The full `dungeon_crawler_cli` sample interprets end-to-end with
//! depth-correct room rendering. Anything outside this subset returns
//! [`InterpretOutcome::error`] so a differential harness SKIPS (xfail) rather than reporting
//! a false mismatch.
//!
//! CASE PAYLOADS are supported in BOTH engines: construction (`Command::Move
//! { steps: 70 }`, the brace spelling shared with record literals), case-pattern
//! binding in transition arms (`Command::Move { steps } -> done(steps)`, with the
//! bound names rewritten to payload member reads), and tag compares against case
//! references (the lowering of `in` and of payload-less case `==`, matching the
//! native 4-byte tag clamp). Structural `==` on CONFORMING types (`Type
//! satisfies Equatable;`) is expanded by the FRONTEND into ordinary field
//! compares and tag-guarded payload compares before either engine runs, so the
//! interpreter's `Value::Enum` equality stays a tag compare -- by the time a
//! payload matters, the expansion already reads it field by field. Expression
//! `&&`/`||` SHORT-CIRCUIT (the expansion relies on it to keep cross-case
//! payload reads unevaluated; the native backend evaluates eagerly but masks
//! the garbage compare behind the false tag guard). Never-assigned sum fields
//! default to the ZII zero case (first case, zeroed payload), matching native
//! zero-initialized storage. The native backend lowers construction as a
//! tag-prefix write plus payload field writes, so payload coverage runs
//! differentially via the `data/case_*` and `traits/equatable_*` RUN canaries
//! (plus the deeper probes in `tests/coverage.rs`).
//!
//! One formerly-deferred construct is FRONTEND-REJECTED today (probed in
//! `tests/coverage.rs`), so there is nothing to interpret:
//! - General/open range expressions outside the index position (`let r: i32 = 1..5;`,
//!   `f(1..5)`) are parse errors; `ExpressionNode::Range` only ever appears under
//!   `collection[...]`, which the subslice support already covers.
//! - (A paren'd construction against a payload-less case (`E::A(5)`) still parses as a
//!   CALL but resolves to nothing; the interpreter declines it.)

mod build_evaluation_sponsor;
mod build_time;
mod evaluation;
mod filesystem;
mod filesystem_replay;
mod filesystem_sponsor;
pub mod interpreter;
mod value;

pub use build_evaluation_sponsor::{
    BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION, BuildEvaluationSponsor,
    BuildEvaluationSponsorLimits,
};
pub use build_time::{BuildTimeValue, SelectedBuildTimeBinaryOperator};
pub use filesystem_replay::{
    FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE, FilesystemInputOutputAbsentRemovesReplayRecord,
    FilesystemInputOutputDirectoryReplayRecord, FilesystemInputOutputTreeReplayRecord,
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    FilesystemInputUnknownDescriptorOpenAtReplayRecord,
    FilesystemInputUnknownDescriptorOperationReplayKind,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord,
    FilesystemInputUnknownDescriptorReadDirReplayRecord,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    FilesystemInputUnknownDescriptorReadReplayKind,
    FilesystemInputUnknownDescriptorReadReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorUnlinkAtReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    FilesystemInputUnknownDescriptorWriteReplayKind,
    FilesystemInputUnknownDescriptorWriteReplayRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
    FilesystemInputUnknownNativeHandleMutationReplayKind,
    FilesystemInputUnknownNativeHandleMutationReplayRecord,
    FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord,
    FilesystemNativeHandleErrorObservationReplayRecord,
    FilesystemNativeHandleFinalPathQueryReplayRecord,
    FilesystemNativeHandleQueryOperationReplayRecord, FilesystemOutputAbsentRemoveKind,
    FilesystemOutputAbsentRemoveReplayRecord, FilesystemOutputChangeFileOwnerReplayRecord,
    FilesystemOutputDirectoryReplayRecord, FilesystemOutputDuplicateReplayRecord,
    FilesystemOutputHardLinkReplayKind, FilesystemOutputHardLinkReplayRecord,
    FilesystemOutputLockReplayRecord, FilesystemOutputSymlinkReplayRecord,
    FilesystemOutputTreeEntryReplayRecord, FilesystemSourceDirectoryReadChainReplayRecord,
    FilesystemSourceDirectoryReadReplayRecord, FilesystemSourceNativeHandleQueryChainReplayRecord,
    FilesystemSourceReadLinkReplayRecord, FilesystemSourceWriteRefusalReplayKind,
    FilesystemSourceWriteRefusalReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS,
    MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES,
};
pub use filesystem_sponsor::{
    COMPILER_DEFAULT_STAGING_ENTRY_LIMIT, COMPILER_DEFAULT_STAGING_MAX_OBJECT_EXTENT,
    COMPILER_DEFAULT_STAGING_TOTAL_LOGICAL_BYTES, FilesystemOpenDescriptor, FilesystemSponsor,
    FilesystemSponsorEntry, FilesystemSponsorError, FilesystemSponsorLimits,
    FilesystemSponsorNamespaceEntry, FilesystemSponsorNamespaceEntryKind,
    FilesystemSponsorNamespaceSnapshot, FilesystemSponsorPath, FilesystemSponsorSnapshot,
    PreparedFilesystemMutation, PreparedFilesystemOpen, PreparedFilesystemWrite,
};
pub use value::{Cell, Value};

pub use evaluation::{
    BuildIncludedSource, BuildMachineEvaluationFailure, BuildMachineEvaluationFailureKind,
    BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt,
    BuildTimeOperationEvaluation, CURRENT_EVALUATION_SEMANTICS, CURRENT_EVALUATION_STEP_SCHEDULE,
    CURRENT_EVALUATION_USAGE_SCHEMA, DescribedProductEntry, DescribedProductProvider,
    DescribedProductSchema, EvaluationObservations, EvaluationSemanticsIdentity,
    EvaluationStepScheduleIdentity, EvaluationUsage, EvaluationUsageSchemaIdentity,
    ExecutedBehaviorExclusion, ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite,
    ExecutedRootBinding, InterpretOutcome, MAX_BUILD_OUTPUT_OBLIGATIONS,
    MeasuredBuildMachineEvaluation, MeasuredEvaluation, PrivateLayoutPlacementReceipt,
};
pub use filesystem::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataIndexError,
    CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind,
    FILESYSTEM_METADATA_API_CARRIER_BYTES, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, FilesystemAccess, FilesystemAuthorizedPath,
    FilesystemByteOperand, FilesystemEvaluationHaltKind, FilesystemGrantAccess,
    FilesystemGrantRefusal, FilesystemGrantRefusalReason, FilesystemGrantRoot,
    FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource, FilesystemMetadataField,
    FilesystemMetadataFieldLayout, FilesystemMetadataLayout, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemMutableI64Operand,
    FilesystemMutableI64OperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemPathLikeOperand,
    FilesystemReturnedPath, FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
    FilesystemServiceBinding, FsGrants, canonical_filesystem_metadata_path_is_canonical,
    filesystem_root_relative_path_is_canonical,
};
pub use filesystem_replay::{
    FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE, FilesystemInputOutputReplayRecord,
    FilesystemOutputFileOperationReplayRecord, FilesystemOutputFileReplayRecord,
    FilesystemOutputWriteReplayKind, FilesystemOutputWriteReplayRecord, FilesystemReplay,
    FilesystemReplayReadKind, FilesystemReplayReadRecord,
    FilesystemSourceDescriptorMetadataReplayRecord, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourcePathMetadataReplayRecord,
    FilesystemSourceReadChainReplayRecord, MAX_FILESYSTEM_REPLAY_RETAINED_BYTES,
    MAX_INCLUDED_BUILD_SOURCES,
};
pub(crate) use filesystem_replay::{
    output_file_attempt_count, source_attempts_overlap_output, source_descriptor_close_attempt,
    source_descriptor_open_attempt, source_input_record_attempts, validate_filesystem_replay_size,
    validate_output_replay_extents, validate_output_time_replay_retention,
    validate_source_input_attempts,
};
pub use interpreter::{
    BuildMachineEntry, BuildMachineEvaluationRequest, InterpretOptions,
    evaluate_build_machine_arguments, evaluate_build_time_machine, evaluate_const_machine,
    evaluate_const_machine_measured, evaluate_granted_build_machine_arguments, interpret_entry,
};
