/*
    Psi-owned checked-tree and typed-tree interpreter for build-time evaluation
    and differential execution.

    We evaluate checked programs directly on `CheckedTrees` and `TypedTrees`
    above backend lowering. Every storage place is an allocation-owned `Cell`
    so `&mut` references alias the underlying cell across multi-hop calls and
    state transitions, while encoded reference views retain their backing,
    projection path, and physical codec.
*/

/*
    Evaluating programs above target lowering provides an independent semantic
    oracle for differential testing and early build-time evaluation before
    constructs lower to Terminal Psi. Keeping this evaluator in `psi/` with
    explicit virtual or grant-scoped filesystem and virtual clock authorities
    ensures execution never depends on backend target realization or ambient
    host state.
*/

/*
    Start at `interpreter` (`interpret_entry` and
    `evaluate_prepared_build_time_machine`) for worker setup, authority
    selection, and result collection. `evaluation` owns measured usage,
    obligations, sponsor accounting, and virtual clock bindings; `value` owns
    `Value`, `Cell`, and `Reference`; `program_values`, `execution`,
    `execution_expressions`, `execution_scalars`, and `build_machine` implement
    the evaluator steps; and `filesystem` and `filesystem_real` implement the
    virtual, captured, and grant-scoped host filesystem providers.
*/

mod build_machine;
mod evaluation;
mod execution;
mod execution_expressions;
mod execution_scalars;
mod filesystem;
mod filesystem_real;
pub mod interpreter;
mod program_values;
mod value;

pub(crate) use evaluation::{
    build_evaluation_sponsor, build_evaluation_usage, build_time, virtual_clock,
};
pub(crate) use filesystem::{
    filesystem_access, filesystem_observations, filesystem_operation_attempts,
};

pub use build_evaluation_sponsor::{
    BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION, BuildEvaluationSponsor,
    BuildEvaluationSponsorLimits,
};
pub use build_evaluation_usage::capture_checked_build_evaluation_usage;
pub use build_time::SelectedBuildTimeBinaryOperator;
pub use evaluation::{
    BuildIncludedSource, BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt,
    BuildTimeOperationEvaluation, CURRENT_EVALUATION_SEMANTICS, CURRENT_EVALUATION_STEP_SCHEDULE,
    CURRENT_EVALUATION_USAGE_SCHEMA, EvaluationSemanticsIdentity, EvaluationStepScheduleIdentity,
    EvaluationUsage, EvaluationUsageSchemaIdentity, ExecutedBehaviorExclusion,
    ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite, ExecutedBuildSelection,
    InterpretOutcome, MAX_BUILD_OUTPUT_OBLIGATIONS, MAX_INCLUDED_BUILD_SOURCES, MeasuredEvaluation,
    PrivateLayoutPlacementReceipt,
};
pub use evaluation_accounting::{
    BuildEvaluationUsage, BuildEvaluationUsageError, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
pub use filesystem_access::{CapturedFilesystemRoot, FilesystemAccess};
pub use filesystem_authority::*;
pub use filesystem_observations::{
    FILESYSTEM_METADATA_API_CARRIER_BYTES, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind,
};
pub use filesystem_operation_attempts::{
    FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION, FilesystemEvaluationHaltKind,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
};
pub use interpreter::{
    BuildMachineEntry, BuildMachineEvaluationError, BuildMachineEvaluationRequest,
    BuildMachineProgram, InterpretOptions, SelectedBuildTimeMachineBody, UnavailableStaticInput,
    evaluate_prepared_build_time_machine, interpret_entry,
    materialize_zero_initialized_build_time_value, pure_evaluation_step_ceiling,
};
pub use value::{Cell, Reference, Value};
pub use virtual_clock::{
    VirtualClockBinding, VirtualClockOperand, VirtualClockOperation, VirtualClockResult,
};
