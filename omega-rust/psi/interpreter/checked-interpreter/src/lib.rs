/*
    Psi-owned checked-tree and typed-tree interpreter for build-time evaluation.

    We evaluate prepared build-time machines directly on `CheckedTrees` and
    `TypedTrees` above backend lowering. Every storage place is an
    allocation-owned `Cell` so `&mut` references alias the underlying cell
    across multi-hop calls and state transitions, while encoded reference views
    retain their backing, projection path, and physical codec.
*/

/*
    This engine is migration debt: whole-program execution and the corpus
    oracle already run on Terminal Psi, and the remaining build-time jobs move
    there under UNIVERSAL-INTERPRETER-CLOSURE. Build-time evaluation supplies
    only the virtual filesystem; no caller grants host filesystem, clock, or
    standard-input authority here.
*/

/*
    Start at `interpreter` (`evaluate_prepared_build_time_machine`) for worker
    setup and result collection. `evaluation` owns measured usage, obligations,
    and sponsor accounting; `value` owns `Value`, `Cell`, and `Reference`;
    `program_values`, `execution`, `execution_expressions`,
    `execution_scalars`, and `build_machine` implement the evaluator steps; and
    `filesystem` implements the virtual filesystem provider.
*/

mod build_machine;
mod evaluation;
mod execution;
mod execution_expressions;
mod execution_scalars;
mod filesystem;
pub mod interpreter;
mod program_values;
mod value;

pub(crate) use evaluation::{build_evaluation_sponsor, build_evaluation_usage, build_time};
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
    BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt,
    BuildTimeOperationEvaluation, CURRENT_EVALUATION_SEMANTICS, CURRENT_EVALUATION_STEP_SCHEDULE,
    CURRENT_EVALUATION_USAGE_SCHEMA, EvaluationSemanticsIdentity, EvaluationStepScheduleIdentity,
    EvaluationUsage, EvaluationUsageSchemaIdentity, ExecutedBehaviorExclusion,
    ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite, ExecutedBuildSelection,
    MAX_BUILD_OUTPUT_OBLIGATIONS, MeasuredEvaluation, PrivateLayoutPlacementReceipt,
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
    BuildMachineProgram, SelectedBuildTimeMachineBody, UnavailableStaticInput,
    evaluate_prepared_build_time_machine, materialize_zero_initialized_build_time_value,
    pure_evaluation_step_ceiling,
};
pub use value::{Cell, Reference, Value};
