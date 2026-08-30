use crate::build_evaluation_sponsor::BuildEvaluationLiveFilesystemHandleLease;
use crate::{
    BuildEvaluationSponsor, BuildMachineEvaluationFailure, BuildMachineEvaluationFailureKind,
    BuildTimeOperationEvaluation, EvaluationObservations, EvaluationUsage, FilesystemAccess,
    FilesystemEvaluationHaltKind, FilesystemGrantAccess, FilesystemGrantRefusal,
    FilesystemGrantRefusalReason, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource, FilesystemMetadataLayout,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, InterpretOptions, InterpretOutcome, MeasuredBuildMachineEvaluation,
    MeasuredEvaluation, PrivateLayoutPlacementReceipt,
};

mod filesystem_host_operation;
use filesystem_host_operation::{FilesystemHostOperation, FilesystemHostResultKind};

#[path = "evaluator/filesystem_logical_handles.rs"]
mod filesystem_logical_handles;
use filesystem_logical_handles::FilesystemLogicalHandles;

#[path = "evaluator/filesystem_preparation.rs"]
mod filesystem_preparation;
use filesystem_preparation::{
    FIND_DATA_OUTPUT_BYTES, PreparedByteOutput, PreparedFilesystemCall,
    PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
    PreparedFilesystemMutableObservationPlan, PreparedFilesystemPreparation, STAT_OUTPUT_BYTES,
    synthetic_handle_fd,
};

#[path = "evaluator/build_log.rs"]
mod build_log;

/// The REAL-filesystem provider (opt-in `FilesystemAccess::RealUnscoped`; the
/// build.omg rung). A CHILD module so it can serve ops against the private
/// `Evaluator` internals (the fs argument/buffer helpers) without widening
/// their visibility; `#[path]` keeps the flat one-file-per-module layout.
#[path = "evaluator_real_fs.rs"]
mod real_fs;

/// Per-target open-flag BIT POSITIONS, mirroring the checked target encoders in
/// `std/targets/<target>/filesystem_impl.omg`. The differential oracle compiles
/// for `host()` and runs ON the host, so selecting by `cfg!(target_os)` needs no
/// target threading. The create/open differential canaries guard this mirror.
/// Access mode (O_WRONLY 1 / O_RDWR 2, mask 0x3) is universal.
mod host_open_flags {
    #[cfg(target_os = "windows")]
    pub const O_CREAT_BIT: i32 = 8;
    #[cfg(target_os = "windows")]
    pub const O_EXCL_BIT: i32 = 10;
    #[cfg(target_os = "windows")]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(target_os = "windows")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(target_os = "macos")]
    pub const O_CREAT_BIT: i32 = 9;
    #[cfg(target_os = "macos")]
    pub const O_EXCL_BIT: i32 = 11;
    #[cfg(target_os = "macos")]
    pub const O_TRUNC_BIT: i32 = 10;
    #[cfg(target_os = "macos")]
    pub const O_APPEND_BIT: i32 = 3;

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_CREAT_BIT: i32 = 6;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_EXCL_BIT: i32 = 7;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_TRUNC_BIT: i32 = 9;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub const O_APPEND_BIT: i32 = 10;

    pub const fn o_creat(flags: i32) -> bool {
        (flags >> O_CREAT_BIT) & 1 != 0
    }
    pub const fn o_excl(flags: i32) -> bool {
        (flags >> O_EXCL_BIT) & 1 != 0
    }
    pub const fn o_trunc(flags: i32) -> bool {
        (flags >> O_TRUNC_BIT) & 1 != 0
    }
    pub const fn o_append(flags: i32) -> bool {
        (flags >> O_APPEND_BIT) & 1 != 0
    }
}
use crate::value::{Cell, CellMeter, Value};
use psi_checked_trees::{CheckedOperatorFacts, CheckedTrees};
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::bignum::BigInt;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_numerics::float_semantics::{
    FloatClass as SemanticFloatClass, FloatFormat as SemanticFloatFormat, FloatMeaning,
    FloatPolicyTrap, FloatSemantics, FloatToIntegerError, IntegerFormat as SemanticIntegerFormat,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember};
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

const STEP_BUDGET: u64 = 10_000_000;

fn ambient_step_budget() -> u64 {
    std::env::var("OMEGA_INTERP_STEP_BUDGET")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(STEP_BUDGET)
}

fn project_landed_float(format: SemanticFloatFormat, value: f64) -> FloatMeaning {
    if format == SemanticFloatFormat::BINARY32 {
        FloatProjectionOperation::Meaning32
            .project_f32(value as f32)
            .expect("binary32 projection row accepts f32")
    } else {
        FloatProjectionOperation::Meaning64
            .project_f64(value)
            .expect("binary64 projection row accepts f64")
    }
}

fn build_time_result_custody(values: &[crate::build_time::BuildTimeValue]) -> Option<(u64, u64)> {
    values
        .iter()
        .try_fold((0u64, 0u64), |(cells, text_bytes), value| {
            Some((
                cells.checked_add(value.retained_cell_count()?)?,
                text_bytes.checked_add(value.retained_text_byte_count()?)?,
            ))
        })
}
/// Fuel cap for CONST EVALUATION (comptime stage 1). The language's
/// termination discipline (no general recursion, loops carry decreases) is the
/// real guarantee; this cap is defense-in-depth against checker gaps. Exceeding
/// it is a compile error at the const site.
const CONST_EVAL_STEP_BUDGET: u64 = 100_000;
/// Max native recursion depth (call / cross-machine transition nesting) before we decline
/// rather than overflow the host stack. Deep recursive programs are skipped (reported as
/// unsupported), never crash the differential harness.
const CALL_DEPTH_BUDGET: u32 = 512;
/// Aggregate byte custody for immutable, path-like, rooted-resolution,
/// returned-path, and mutable filesystem evidence retained during one
/// evaluator run. A successful mutable byte call retains resolution, provider
/// pre-state, and provider post-state under this same sponsor. Individual
/// prepared carriers remain bounded by their separate 16 MiB evaluator limit.
const MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES: usize = 256 * 1024 * 1024;

/// The modeled `st_mtime` (seconds since the Unix epoch) the hermetic virtual
/// filesystem reports for every entry — it has no real clock. A recognizable
/// round value (2001-09-09T01:46:40Z). Native `stat` returns the real time.
/// The canonical metadata observation constructor supplies distinct modeled
/// values for the remaining timestamps and identity/allocation fields.
const VIRTUAL_MTIME_SECS: i64 = 1_000_000_000;
/// The hermetic ownership mutation model uses the same fixed identities as the
/// canonical metadata observation.
const VIRTUAL_UID: u32 = 501;
const VIRTUAL_GID: u32 = 20;

pub(crate) fn run(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
) -> InterpretOutcome {
    run_with_options(
        checked,
        entry_machine_name,
        stdin,
        InterpretOptions::default(),
    )
}

pub(crate) fn run_with_options(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    // Run on a worker thread with a generous stack: the tree-walker recurses with the
    // program's call/expression nesting, which can exceed the default test-thread stack on
    // deep programs even with the call-depth budget. A scoped thread lets us keep the
    // borrow of `checked`/`stdin`.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                run_on_current_thread(checked, entry_machine_name, stdin, options)
            })
            .expect("spawn interpreter worker thread")
            .join()
            .unwrap_or_else(|_| {
                InterpretOutcome::error(
                    "interpreter thread panicked",
                    Vec::new(),
                    Vec::new(),
                    EvaluationUsage::empty(ambient_step_budget()),
                )
            })
    })
}

/// BUILD-TIME EVALUATION (stage 1): run a zero-argument, effect-free
/// machine to its terminal value and return that value as an `i64`, width-
/// adjusted to the machine's declared integer return type (the same
/// `wrap_to_width` the interpreter applies on writes, so the result is
/// TARGET-width-correct, not host-width). The caller (the compiler's
/// const-eval pass) owns the purity gate; this entry owns evaluation and a
/// small evaluator-step ceiling. Errors carry a human-readable reason for the
/// compile diagnostic at the const site.
pub(crate) fn run_const_machine(
    program: &TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                run_const_machine_on_current_thread(program, machine_name)
            })
            .expect("spawn const-eval worker thread")
            .join()
            .unwrap_or_else(|_| Err("const evaluator thread panicked".to_owned()))
    })
}

fn run_const_machine_on_current_thread(
    program: &TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    let mut evaluator = Evaluator::new(program, &[]);
    evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, None);
    let result = evaluator.run_const_machine(machine_name);
    evaluator.finish_cell_usage();
    let mut usage = evaluator.usage;
    match result {
        Ok(value) => {
            usage.record_result_custody(1, 0);
            Ok(MeasuredEvaluation::new(value, usage))
        }
        Err(Halt::Exit(code)) => Err(format!(
            "the machine attempted to exit the process (code {code}) instead of returning a value"
        )),
        Err(Halt::Unsupported(message))
        | Err(Halt::Trap(message))
        | Err(Halt::Resource(message)) => Err(message),
    }
}

/// STRUCTURED build-time evaluation (the R2 layouts enabler): run an
/// effect-free machine with compiler-built ARGUMENTS and read back its
/// terminal value as a structured tree. Same ownership split as
/// `run_const_machine`: the caller owns the purity gate (decision 12's
/// transitive effect surface), this entry owns evaluation + the evaluator-step
/// ceiling.
pub(crate) fn run_build_time_machine(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<MeasuredEvaluation<crate::build_time::BuildTimeValue>, String> {
    run_build_time_machine_with_operation_receipts(program, machine_name, arguments).map(
        |evaluation| {
            let (value, usage, _) = evaluation.into_parts();
            MeasuredEvaluation::new(value, usage)
        },
    )
}

pub(crate) fn run_build_time_machine_with_operation_receipts(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<BuildTimeOperationEvaluation<crate::build_time::BuildTimeValue>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, None);
                let result = evaluator.run_build_time_machine(machine_name, arguments);
                evaluator.finish_cell_usage();
                let mut usage = evaluator.usage;
                match result {
                    Ok(value) => {
                        let (result_cells, result_text_bytes) =
                            build_time_result_custody(std::slice::from_ref(&value)).ok_or_else(|| {
                            "build-time evaluator result-cell count overflowed".to_owned()
                        })?;
                        usage.record_result_custody(result_cells, result_text_bytes);
                        Ok(BuildTimeOperationEvaluation::new(
                            value,
                            usage,
                            evaluator.private_layout_placements,
                        ))
                    }
                    Err(Halt::Exit(code)) => Err(format!(
                        "the machine attempted to exit the process (code {code}) instead of returning a value"
                    )),
                    Err(Halt::Unsupported(message))
                    | Err(Halt::Trap(message))
                    | Err(Halt::Resource(message)) => Err(message),
                }
            })
            .expect("spawn build-time evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("build-time evaluator thread panicked".to_owned()))
    })
}

/// The AUGMENTING-MACHINE build-time entry (build_and_package_model.md):
/// evaluate `machine_name` with the given arguments and read back the FINAL
/// argument values -- the `machine build(b: &mut Build)` shape, where the
/// machine augments a passed-in value and returns nothing. The terminal value
/// (if any) is discarded; a unit machine is fine.
pub(crate) fn run_build_time_machine_arguments(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<MeasuredEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    run_observed_build_time_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        None,
    )
    .map(|measured| {
        let (value, usage, _) = measured.into_parts();
        MeasuredEvaluation::new(value, usage)
    })
}

pub(crate) fn run_build_time_machine_arguments_with_sponsor(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    run_observed_build_time_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        Some(sponsor.clone()),
    )
    .map(|measured| {
        let (value, usage, _) = measured.into_parts();
        MeasuredEvaluation::new(value, usage)
    })
}

pub(crate) fn run_observed_build_time_machine_arguments(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    run_observed_build_time_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        None,
    )
}

pub(crate) fn run_observed_build_time_machine_arguments_with_sponsor(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    run_observed_build_time_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        Some(sponsor.clone()),
    )
}

fn run_observed_build_time_machine_arguments_with_optional_sponsor(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    sponsor: Option<BuildEvaluationSponsor>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, sponsor);
                let result = evaluator.run_build_time_machine_arguments(machine_name, arguments);
                evaluator.finish_cell_usage();
                use std::io::Write as _;
                if !evaluator.build_log.is_empty() {
                    let _ = std::io::stdout().write_all(&evaluator.build_log);
                    let _ = std::io::stdout().flush();
                }
                let mut usage = evaluator.usage;
                let observations = EvaluationObservations::from_build_run(
                    Vec::new(),
                    Vec::new(),
                    std::mem::take(&mut evaluator.build_log),
                );
                match result {
                    Ok(values) => {
                        let (result_cells, result_text_bytes) =
                            build_time_result_custody(&values).ok_or_else(|| {
                            "build-time evaluator result-cell count overflowed".to_owned()
                        })?;
                        if let Some(sponsor) = &evaluator.build_evaluation_sponsor {
                            sponsor.charge_result_custody(result_cells, result_text_bytes)?;
                        }
                        usage.record_result_custody(result_cells, result_text_bytes);
                        Ok(MeasuredBuildMachineEvaluation::new(
                            values,
                            usage,
                            observations,
                        ))
                    }
                    Err(Halt::Exit(code)) => Err(format!(
                        "the machine attempted to exit the process (code {code}) instead of returning"
                    )),
                    Err(Halt::Unsupported(message))
                    | Err(Halt::Trap(message))
                    | Err(Halt::Resource(message)) => Err(message),
                }
            })
            .expect("spawn build-time evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("build-time evaluator thread panicked".to_owned()))
    })
}

/// The GRANTED build entry (open-work #3 rung 4, interpreter side): run the
/// augmenting `build(b: &mut Build)` machine WITH a filesystem capability --
/// virtual (hermetic tests) or real scoped/unscoped per `options` -- and read
/// back the augmented arguments. Filesystem ops are allowed (the grant is the
/// audit surface); any OTHER host boundary (console, clock, gui) rejects.
/// Runs under the FULL step budget: staging assets is real work, unlike the
/// const-eval step ceiling the pure entry rides.
pub(crate) fn run_granted_build_machine_arguments(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    options: InterpretOptions,
) -> Result<
    MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>,
    BuildMachineEvaluationFailure,
> {
    run_granted_build_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        options,
        None,
    )
}

pub(crate) fn run_granted_build_machine_arguments_with_sponsor(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    options: InterpretOptions,
    sponsor: &BuildEvaluationSponsor,
) -> Result<
    MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>,
    BuildMachineEvaluationFailure,
> {
    run_granted_build_machine_arguments_with_optional_sponsor(
        program,
        machine_name,
        arguments,
        options,
        Some(sponsor.clone()),
    )
}

fn run_granted_build_machine_arguments_with_optional_sponsor(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    options: InterpretOptions,
    sponsor: Option<BuildEvaluationSponsor>,
) -> Result<
    MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>,
    BuildMachineEvaluationFailure,
> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.configure_build_evaluation(STEP_BUDGET, sponsor);
                evaluator.filesystem_metadata_layout = options.filesystem_metadata_layout;
                let replaying = matches!(
                    &options.filesystem,
                    FilesystemAccess::ReplayFilesystem(_)
                );
                match options.filesystem {
                    FilesystemAccess::Virtual => {}
                    FilesystemAccess::RealUnscoped => {
                        evaluator.real_fs = Some(
                            real_fs::RealFs::new(None, None)
                                .expect("unscoped filesystem has no grant configuration"),
                        );
                    }
                    FilesystemAccess::RealScoped(grants) => {
                        evaluator.real_fs = Some(real_fs::RealFs::new(Some(grants), None).map_err(
                            |message| {
                                BuildMachineEvaluationFailure::without_evidence(
                                    BuildMachineEvaluationFailureKind::InvalidFilesystemGrant,
                                    message,
                                )
                            },
                        )?);
                    }
                    FilesystemAccess::RealScopedSponsored { grants, sponsor } => {
                        evaluator.real_fs = Some(
                            real_fs::RealFs::new(Some(grants), Some(sponsor)).map_err(
                                |message| {
                                    BuildMachineEvaluationFailure::without_evidence(
                                        BuildMachineEvaluationFailureKind::InvalidFilesystemGrant,
                                        message,
                                    )
                                },
                            )?,
                        );
                    }
                    FilesystemAccess::ReplayFilesystem(replay) => {
                        evaluator.filesystem_replay = Some(replay);
                    }
                }
                let result = evaluator.run_build_machine_arguments_with_policy(
                    machine_name,
                    arguments,
                    true,
                ).and_then(|values| {
                    evaluator.finish_filesystem_replay()?;
                    Ok(values)
                });
                evaluator.finish_cell_usage();
                // Build logging reaches the REAL streams (owner answer #5:
                // "the interpreter should never just catch it") -- including
                // on failure, where the partial log is the diagnostic.
                use std::io::Write as _;
                if !replaying && !evaluator.stdout.is_empty() {
                    let _ = std::io::stdout().write_all(&evaluator.stdout);
                    let _ = std::io::stdout().flush();
                }
                if !replaying && !evaluator.stderr.is_empty() {
                    let _ = std::io::stderr().write_all(&evaluator.stderr);
                    let _ = std::io::stderr().flush();
                }
                if !replaying && !evaluator.build_log.is_empty() {
                    let _ = std::io::stdout().write_all(&evaluator.build_log);
                    let _ = std::io::stdout().flush();
                }
                let mut usage = evaluator.usage;
                let observations = EvaluationObservations::from_build_run(
                    std::mem::take(&mut evaluator.filesystem_operation_attempts),
                    std::mem::take(&mut evaluator.build_included_sources),
                    std::mem::take(&mut evaluator.build_log),
                );
                match result {
                    Ok(values) => {
                        let Some((result_cells, result_text_bytes)) =
                            build_time_result_custody(&values)
                        else {
                            return Err(BuildMachineEvaluationFailure::with_evidence(
                                BuildMachineEvaluationFailureKind::ResultAccountingOverflow,
                                "build-time evaluator result-cell count overflowed".to_owned(),
                                usage,
                                observations,
                            ));
                        };
                        if let Some(sponsor) = &evaluator.build_evaluation_sponsor
                            && let Err(message) = sponsor
                                .charge_result_custody(result_cells, result_text_bytes)
                        {
                            return Err(BuildMachineEvaluationFailure::with_evidence(
                                BuildMachineEvaluationFailureKind::ResourceExhausted,
                                message,
                                usage,
                                observations,
                            ));
                        }
                        usage.record_result_custody(result_cells, result_text_bytes);
                        Ok(MeasuredBuildMachineEvaluation::new(
                            values,
                            usage,
                            observations,
                        ))
                    }
                    Err(Halt::Exit(code)) => Err(BuildMachineEvaluationFailure::with_evidence(
                        BuildMachineEvaluationFailureKind::Exit,
                        format!(
                            "the machine attempted to exit the process (code {code}) instead of returning"
                        ),
                        usage,
                        observations,
                    )),
                    Err(Halt::Unsupported(message)) => {
                        Err(BuildMachineEvaluationFailure::with_evidence(
                            BuildMachineEvaluationFailureKind::Unsupported,
                            message,
                            usage,
                            observations,
                        ))
                    }
                    Err(Halt::Trap(message)) => {
                        Err(BuildMachineEvaluationFailure::with_evidence(
                            BuildMachineEvaluationFailureKind::Trap,
                            message,
                            usage,
                            observations,
                        ))
                    }
                    Err(Halt::Resource(message)) => {
                        Err(BuildMachineEvaluationFailure::with_evidence(
                            BuildMachineEvaluationFailureKind::ResourceExhausted,
                            message,
                            usage,
                            observations,
                        ))
                    }
                }
            })
            .map_err(|error| {
                BuildMachineEvaluationFailure::without_evidence(
                    BuildMachineEvaluationFailureKind::WorkerUnavailable,
                    format!("failed to spawn granted build evaluator thread: {error}"),
                )
            })?;
        worker.join().unwrap_or_else(|_| {
            Err(BuildMachineEvaluationFailure::without_evidence(
                BuildMachineEvaluationFailureKind::WorkerPanicked,
                "granted build evaluator thread panicked".to_owned(),
            ))
        })
    })
}

fn run_on_current_thread(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    let mut evaluator = Evaluator::new_checked(checked, stdin);
    if checked
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Atomic(atomic)
                    if matches!(
                        atomic.ordering,
                        psi_language_core::AtomicOrderingPlan::CompareExchangeOnce { .. }
                    )
            )
        })
    {
        return InterpretOutcome::error(
            "observing single-attempt compare-exchange has no runtime result carrier".to_owned(),
            evaluator.stdout,
            evaluator.stderr,
            evaluator.usage,
        );
    }
    evaluator.filesystem_metadata_layout = options.filesystem_metadata_layout;
    match options.filesystem {
        FilesystemAccess::Virtual => {}
        FilesystemAccess::RealUnscoped => {
            evaluator.real_fs = Some(
                real_fs::RealFs::new(None, None)
                    .expect("unscoped filesystem has no grant configuration"),
            );
        }
        FilesystemAccess::RealScoped(grants) => {
            let filesystem = match real_fs::RealFs::new(Some(grants), None) {
                Ok(filesystem) => filesystem,
                Err(message) => {
                    return InterpretOutcome::error(
                        message,
                        evaluator.stdout,
                        evaluator.stderr,
                        evaluator.usage,
                    );
                }
            };
            evaluator.real_fs = Some(filesystem);
        }
        FilesystemAccess::RealScopedSponsored { grants, sponsor } => {
            let filesystem = match real_fs::RealFs::new(Some(grants), Some(sponsor)) {
                Ok(filesystem) => filesystem,
                Err(message) => {
                    return InterpretOutcome::error(
                        message,
                        evaluator.stdout,
                        evaluator.stderr,
                        evaluator.usage,
                    );
                }
            };
            evaluator.real_fs = Some(filesystem);
        }
        FilesystemAccess::ReplayFilesystem(replay) => {
            evaluator.filesystem_replay = Some(replay);
        }
    }
    let result = evaluator
        .run_entry(entry_machine_name)
        .and_then(|()| evaluator.finish_filesystem_replay());
    evaluator.finish_cell_usage();
    let usage = evaluator.usage;
    match result {
        Ok(()) => {
            // Reached a terminal transition without an explicit exit_process.
            InterpretOutcome::exited(0, evaluator.stdout, evaluator.stderr, usage)
        }
        Err(Halt::Exit(code)) => {
            InterpretOutcome::exited(code, evaluator.stdout, evaluator.stderr, usage)
        }
        Err(Halt::Unsupported(message))
        | Err(Halt::Trap(message))
        | Err(Halt::Resource(message)) => {
            InterpretOutcome::error(message, evaluator.stdout, evaluator.stderr, usage)
        }
    }
}

#[cfg(test)]
mod atomic_fence_tests {
    use super::run_on_current_thread;
    use crate::InterpretOptions;
    use psi_checked_trees::CheckedTrees;
    use psi_checked_trees::expression::{ExpressionNode, TableAtomicExpression};
    use psi_language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
    use psi_numerics::literals::IntegerLiteral;

    #[test]
    fn checked_interpreter_rejects_single_attempt_compare_exchange_before_execution() {
        let mut checked = CheckedTrees::default();
        let value = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        checked
            .typed
            .expression_table
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result: value,
                ordering: AtomicOrderingPlan::CompareExchangeOnce {
                    success: MemoryOrdering::ReceivePublish,
                    failure: MemoryOrdering::Receive,
                },
            }));

        let outcome =
            run_on_current_thread(&checked, "Main.main", &[], InterpretOptions::default());

        assert_eq!(
            outcome.error.as_deref(),
            Some("observing single-attempt compare-exchange has no runtime result carrier")
        );
    }
}

/// A non-local control-flow signal. `Exit` halts cleanly with a code; the others abort
/// the run and surface as `InterpretOutcome.error` (so a harness skips rather than
/// reports a false mismatch).
enum Halt {
    Exit(i32),
    Unsupported(String),
    Trap(String),
    Resource(String),
}

type EvalResult<T> = Result<T, Halt>;

/// Pack `(name, d_type)` entries as darwin `dirent` records, the layout native
/// `___getdirentries64` returns (reclen u16 @16, namlen u16 @18, d_type u8
/// @20, name @21, records 8-byte aligned) -- so a parser is identical on both
/// engines. Shared by the virtual fs (`build_dirent_records`) and the real-fs
/// provider (`try_real_filesystem_call`'s `read_dir`), which differ only in
/// where the names come from.
fn pack_dirent_records(entries: &[(Vec<u8>, u8)]) -> Vec<u8> {
    let mut buffer = Vec::new();
    for (name, d_type) in entries {
        let namlen = name.len();
        let reclen = (25 + namlen).div_ceil(8) * 8;
        let start = buffer.len();
        buffer.resize(start + reclen, 0);
        buffer[start + 16..start + 18].copy_from_slice(&(reclen as u16).to_le_bytes());
        buffer[start + 18..start + 20].copy_from_slice(&(namlen as u16).to_le_bytes());
        buffer[start + 20] = *d_type;
        buffer[start + 21..start + 21 + namlen].copy_from_slice(name);
    }
    buffer
}

/// Select the next complete-record window from a packed Darwin dirent stream.
/// `getdirentries64` never splits a record across caller buffers, so the
/// interpreter advances its synthetic byte cursor only through the last record
/// that fits in `count` bytes. The std wrapper uses a 512-byte buffer, larger
/// than the maximum packed record produced above.
fn dirent_record_chunk(records: &[u8], start: usize, count: usize) -> (&[u8], usize) {
    if start >= records.len() || count == 0 {
        return (&records[0..0], start);
    }

    let limit = start.saturating_add(count).min(records.len());
    let mut end = start;
    while end + 18 <= records.len() {
        let reclen = u16::from_le_bytes([records[end + 16], records[end + 17]]) as usize;
        if reclen == 0 || end + reclen > records.len() || end + reclen > limit {
            break;
        }
        end += reclen;
    }
    (&records[start..end], end)
}

fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Unsupported(message.into()))
}

fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Trap(message.into()))
}

fn filesystem_sponsor_halt<T>(error: crate::FilesystemSponsorError) -> EvalResult<T> {
    let message = format!("filesystem staging sponsor rejected operation: {error}");
    if error.is_limit_exceeded() {
        Err(Halt::Resource(message))
    } else {
        Err(Halt::Trap(message))
    }
}

#[derive(Clone)]
enum MutableScalarRecast {
    Direct {
        source: PrimitiveType,
        target: PrimitiveType,
    },
    ByteRegion {
        cells: Vec<Cell>,
        offset: usize,
        target: PrimitiveType,
    },
    AggregateByteRegion {
        cells: Vec<Cell>,
        offset: usize,
        target_type: TypeReferenceHandle,
    },
    AggregateTyped {
        source: Cell,
        source_type: TypeReferenceHandle,
        target_type: TypeReferenceHandle,
    },
}

impl MutableScalarRecast {
    fn target(&self) -> Option<PrimitiveType> {
        match self {
            Self::Direct { target, .. } | Self::ByteRegion { target, .. } => Some(*target),
            Self::AggregateByteRegion { .. } | Self::AggregateTyped { .. } => None,
        }
    }
}

#[derive(Clone)]
enum MutableRecordProjectionStep {
    Field(String),
    Index(ExpressionHandle),
}

#[derive(Clone, Copy)]
struct MutableRecordProjection {
    offset: usize,
    type_reference: TypeReferenceHandle,
    stored_integer: Option<psi_typed_trees::PlanLaidIntegerField>,
}

/// A lexical scope: parameter / local bindings by name, plus the receiver (`self`) cell.
/// `locals` is behind a `RefCell` so `let` bindings can be added while the frame is
/// shared by `&` during statement execution.
struct Frame {
    locals: RefCell<BTreeMap<String, Cell>>,
    type_locals: RefCell<BTreeMap<String, TypeReferenceHandle>>,
    /// DECLARED scalar (primitive, arithmetic-domain) of locals/params, recorded
    /// at binding -- the static type witness `Value::Int` alone cannot carry.
    /// Read for two classifications native derives from the same declared types:
    /// u64-classed names (`u64`/`usize`/`addr`) make comparisons UNSIGNED at
    /// width 8 (`Value::Int` cannot distinguish u64::MAX from -1), and
    /// Saturating/Trapping names make arithmetic NODES clamp/trap at the
    /// operation itself (native emits the saturating ADD; a landing-seam
    /// coercion alone cannot represent an expression whose own domain differs
    /// from its landing slot's).
    scalar_locals: RefCell<BTreeMap<String, (PrimitiveType, ArithmeticDomain)>>,
    /// Mutable recast locals retain either one equal-width scalar cell or an
    /// indexed byte region. The local remains a normal `Ref` for place
    /// resolution, while these descriptors preserve the stated scalar/record
    /// geometry at the observable read/write seams.
    mutable_scalar_recasts: RefCell<BTreeMap<String, MutableScalarRecast>>,
    self_cell: Cell,
    /// The machine whose state is currently executing. Lets a call/transition that names a
    /// SIBLING state resolve it within this machine (rather than re-entering the machine's
    /// entry state, which would recurse forever).
    machine_symbol: SymbolHandle,
    /// Value-call results computed while evaluating THIS state pass's transition guards,
    /// keyed by call-expression handle. A transition subject is evaluated ONCE per
    /// transition evaluation: the parser lowers `transition self.f(x) { true -> a
    /// false -> b }` into one guard per arm, each holding a COPY of the subject call, so
    /// a later arm must reuse the first arm's result (matching the native lowering)
    /// instead of re-running the callee's side effects. Copies have distinct handles, so
    /// lookups compare structurally. The frame is rebuilt for every state (re)entry, so
    /// loops re-evaluate naturally.
    guard_call_results: RefCell<Vec<(ExpressionHandle, Value)>>,
}

/// One open descriptor in the interpreter's virtual filesystem: which path it
/// refers to, the read/write cursor, and whether it was opened writable.
struct VirtualFd {
    path: Vec<u8>,
    /// Open-file-description cursor shared by every descriptor produced by
    /// `duplicate`, matching POSIX `dup`/Rust `File::try_clone` semantics.
    cursor: std::rc::Rc<std::cell::Cell<usize>>,
    writable: bool,
    /// A descriptor over a DIRECTORY (opened read-only for `read_dir`); a normal
    /// `read`/`write` on it is EISDIR.
    is_dir: bool,
}

struct Evaluator<'program> {
    program: &'program TypedTrees,
    /// Full-program interpretation retains checked named-operator evidence so
    /// a root-preserving intrinsic rewrite can still report the source
    /// operation. Const/build-time evaluation runs before that evidence exists.
    operator_facts: Option<&'program CheckedOperatorFacts>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    /// Exact output emitted through the compiler-owned `Build.log` facet.
    /// This stays distinct from runtime Console output so build evidence
    /// cannot accidentally attribute an ordinary boundary call to BuildLog.
    build_log: Vec<u8>,
    stdin: &'program [u8],
    stdin_cursor: usize,
    /// Virtual monotonic tick counter for `Clock.tick_count` (advances on every
    /// read and every `sleep`); deterministic, so tick-based programs must
    /// assert monotonicity rather than concrete values.
    virtual_ticks: i64,
    /// The virtual window system: `window_create` mints opaque non-zero handle
    /// tokens; `is_window` reports membership; `window_destroy` removes.
    /// Deterministic, so programs must branch on liveness (handle != 0,
    /// is_window > 0), never on concrete handle values.
    virtual_live_windows: std::collections::HashSet<i64>,
    virtual_window_next: i64,
    /// A deterministic in-memory filesystem for `std::fs` programs: no real
    /// disk, so the differential oracle stays reproducible (mirrors the other
    /// `virtual_*` subsystems). `virtual_files` maps a path's bytes to its
    /// content bytes; `virtual_fds` maps an open descriptor to its cursor +
    /// writability. Descriptors start at 3 — 0/1/2 are the standard streams and
    /// are never minted as `File` handles.
    virtual_files: BTreeMap<Vec<u8>, Vec<u8>>,
    virtual_fds: BTreeMap<i32, VirtualFd>,
    virtual_next_fd: i32,
    /// Directories in the virtual filesystem (create_dir/remove_dir).
    virtual_dirs: std::collections::BTreeSet<Vec<u8>>,
    /// Exact checked physical carrier selected for canonical metadata results.
    /// Omega supplies this closed descriptor; Psi never receives target names
    /// or programmable-layout source.
    filesystem_metadata_layout: FilesystemMetadataLayout,
    /// Open find-enumeration cursors (`find_first`/`find_next`/`find_close`,
    /// the windows dir-walk seam ops, fs rung 3a): handle -> the REMAINING
    /// entries (name bytes, is_dir), snapshotted at `find_first` exactly like
    /// a Win32 find handle. Handles start at 1 (-1 is INVALID_HANDLE_VALUE).
    virtual_finds: BTreeMap<i64, std::collections::VecDeque<(Vec<u8>, bool)>>,
    virtual_next_find: i64,
    /// Explicitly-set permission bits per path (`set_permissions`/chmod). A path
    /// absent from this map is treated as writable (the default); only a path
    /// chmod'd to drop the owner-write bit (mode & 0o200 == 0) makes a write-open
    /// fail with EACCES — enough to model `set_permissions` without tracking a
    /// mode for every created file.
    virtual_perms: BTreeMap<Vec<u8>, u32>,
    /// Symbolic links: link path -> target bytes (`symlink`/`read_link`). The
    /// hermetic model stores and returns targets but does NOT resolve them on
    /// open/stat (see TASKS_FS.md); native symlinks resolve for real.
    virtual_symlinks: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Explicitly-set modification times: path -> mtime seconds (`set_file_times`
    /// / `File::set_times`). `stat`/`fstat` report this when present, else the fixed
    /// modeled epoch. The hermetic model round-trips MODIFIED time (whole seconds);
    /// access time is set natively but the model reports the fixed modeled atime.
    virtual_times: BTreeMap<Vec<u8>, i64>,
    /// Advisory whole-file locks (`flock` / Rust `File::lock`/`unlock`): path ->
    /// the fd that holds an EXCLUSIVE lock. A non-blocking acquire on a path
    /// another fd already holds returns EWOULDBLOCK; a lock is released by
    /// LOCK_UN or by closing the owning fd. Shared-lock coexistence and real
    /// blocking are documented approximations (a single-threaded run can't
    /// exercise them); exclusive contention is what the model tracks.
    virtual_flocks: BTreeMap<Vec<u8>, i32>,
    /// Character-special device files (`/dev/null` etc.): paths that `stat` reports
    /// with an `S_IFCHR` mode instead of a regular file, so `FileType`/
    /// `FileTypeExt::is_char_device()` resolves the same on both engines. The
    /// hermetic FS has no real device nodes; this seeds the common ones so a
    /// differential test can `metadata("/dev/null")` without special-casing.
    virtual_char_devices: std::collections::BTreeSet<Vec<u8>>,
    /// The thread-local `errno` model: set to a POSIX code when a virtual fs op
    /// fails (ENOENT=2, EACCES=13, EEXIST=17, EBADF=9), read back by
    /// `read_errno` (darwin `___error()`). Mirrors the native seam so the typed
    /// error model (`io::ErrorKind`) resolves identically on both engines.
    virtual_errno: i32,
    /// `Some` iff the run was started with `FilesystemAccess::RealUnscoped`
    /// (build.omg rung 1): every filesystem op is served against the REAL host
    /// filesystem instead of the virtual model above. The default (`None`)
    /// keeps the interpreter hermetic -- the differential oracle never touches
    /// real disk.
    real_fs: Option<real_fs::RealFs>,
    /// Expected compiler-produced events for bounded no-host filesystem replay.
    filesystem_replay: Option<crate::FilesystemReplay>,
    /// The canonical Build activation carried Source/Output facets. In this
    /// mode path-taking host operations require interpreter-retained rooted
    /// provenance; bare byte spellings cannot select a grant root.
    rooted_build_paths_required: bool,
    /// Explicit Output-rooted source coordinates recorded by the exact
    /// toolchain handoff machine. Orchestration validates these against its
    /// captured sponsored tree before using any bytes.
    build_included_sources: Vec<crate::BuildIncludedSource>,
    /// Set whenever a host-boundary call is driven (statement position or the
    /// value-call fallback). The build-time evaluation entry rejects runs that
    /// touched the host: a dynamic backstop behind decision 12's static gate.
    host_boundary_touched: bool,
    /// Like `host_boundary_touched` but EXCLUDING the filesystem family: the
    /// GRANTED build entry (`evaluate_build_machine_with_filesystem`) allows
    /// fs ops (the grant is the audit surface, open-work #3's settled design)
    /// while still rejecting every OTHER host boundary (console, clock, gui)
    /// as its dynamic backstop.
    non_fs_host_boundary_touched: bool,
    /// Ordered operation-attempt evidence for exact canonical filesystem host
    /// calls. Direct scoped path authorizations retain compiler-rooted paths;
    /// typed operands, mutable carriers, and logical handles retain their
    /// completed preparation prefix. Exact path results and file/directory
    /// observation regions and canonical metadata values are designated, but
    /// replay execution remains incomplete.
    filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
    /// Compiler-only normalization state for provider descriptor/handle tokens.
    /// This state is not observable by evaluated Omega code.
    filesystem_logical_handles: FilesystemLogicalHandles,
    /// Active compiler-owned package-build reservations keyed by the logical
    /// resource identity that owns them. Borrowed native views have no entry.
    filesystem_live_handle_leases:
        BTreeMap<crate::FilesystemLogicalHandleIdentity, BuildEvaluationLiveFilesystemHandleLease>,
    /// Aggregate retained authorized rooted-path bytes.
    filesystem_observation_path_bytes: usize,
    /// Aggregate retained immutable, path-like, rooted-resolution,
    /// returned-path, and mutable evidence bytes, including resolution and
    /// provider pre/post copies. Observed-byte regions reference post-state and
    /// add no byte copy. This compiler-side account is not observable by Omega.
    filesystem_observation_evidence_bytes: usize,
    /// Pending non-catchable halt set when retaining a successfully authorized
    /// rooted path would exceed the compiler's evidence-custody bound.
    filesystem_observation_resource_halt: Option<String>,
    /// Stack of call-start indices used to attach nested provider-side facts to
    /// the exact active operation attempt.
    filesystem_operation_attempt_stack: Vec<usize>,
    /// Compiler-only receipts from executed `Plan::place_private` calls. The
    /// values are returned beside build-time evaluation and never enter the
    /// interpreted store.
    private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
    usage: EvaluationUsage,
    /// Exact allocation-lifetime meter for semantic interpreter cells.
    cell_meter: CellMeter,
    /// Optional compiler-owned account shared across a complete build-review
    /// session. This measures deterministic compiler-owned build resources.
    build_evaluation_sponsor: Option<BuildEvaluationSponsor>,
    /// Total step allowance for this run. Full-program interpretation uses
    /// `STEP_BUDGET`; const evaluation uses the much smaller
    /// `CONST_EVAL_STEP_BUDGET` as a defense-in-depth step ceiling.
    step_budget: u64,
    call_depth: u32,
    /// Non-zero while evaluating a transition GUARD expression. Value-calls evaluated
    /// under a guard memoize into the frame's `guard_call_results` so the per-arm
    /// copies of one transition subject evaluate the callee once (see `Frame`).
    guard_depth: u32,
}

#[path = "evaluator/boundary_console.rs"]
mod boundary_console;
#[path = "evaluator/build_paths.rs"]
mod build_paths;
use build_paths::{rooted_build_path_parts, validate_build_relative_path};
#[path = "evaluator/casts_and_recasts.rs"]
mod casts_and_recasts;
#[path = "evaluator/execution.rs"]
mod execution;
#[path = "evaluator/expressions_and_value_calls.rs"]
mod expressions_and_value_calls;
#[path = "evaluator/filesystem.rs"]
mod filesystem;
#[path = "evaluator/host_dispatch.rs"]
mod host_dispatch;
#[path = "evaluator/names_recasts_and_places.rs"]
mod names_recasts_and_places;
#[path = "evaluator/program_lookup.rs"]
mod program_lookup;
#[path = "evaluator/record_views.rs"]
mod record_views;
#[path = "evaluator/scalar_operations.rs"]
mod scalar_operations;
#[path = "evaluator/statements_and_calls.rs"]
mod statements_and_calls;
#[path = "evaluator/type_metadata.rs"]
mod type_metadata;
#[path = "evaluator/wire_codec.rs"]
mod wire_codec;

/// The canonical Console host-boundary method names the interpreter drives directly.
fn is_canonical_host_method(name: &str) -> bool {
    matches!(
        name,
        "write"
            | "write_line"
            | "write_error"
            | "write_error_line"
            | "read_line"
            | "read_byte"
            | "write_byte"
            | "exit_process"
            | "sleep"
            | "tick_count"
            | "key_state"
            | "dc_create"
            | "get_dc"
            | "window_create"
            | "blit"
            | "msg_peek"
            | "msg_translate"
            | "msg_dispatch"
            | "is_window"
            | "window_destroy"
            | "foreground_window"
    )
}

/// Reinterpret an i64 at an integer primitive's width, sign- or zero-extending back to i64
/// so the value carries the same numeric meaning the target type would observe. `u8` 250
/// stays 250; `i8` 250 wraps to -6; `u32` of a negative becomes its 32-bit unsigned value.
/// zigzag(n) = (n << 1) ^ (n >> 63): the signed-scalar pre-step of the
/// compact_binary v0 varint, identical to the native encoders' shift/xor.
/// One CURRENT-era field of a wire schema, as the interpreter's encoder sees
/// it: a directly encodable scalar/String, or a nested message's scalar-only
/// field list (chapter 20).
enum WireInterpField {
    Direct(psi_typed_trees::wire::WireFieldEncoding),
    Nested(Vec<(String, u64, psi_typed_trees::wire::WireScalarEncoding)>),
    Repeated(psi_typed_trees::wire::WireRepeatedEncoding),
    ScalarSlice(psi_typed_trees::wire::WireBorrowedScalarSliceEncoding),
    /// A borrowed byte slice `&[u8]`: encodes as RAW bytes (length varint then
    /// the bytes), reading the field's element array.
    ByteSlice,
}

/// One CURRENT-era field of a wire schema, as the interpreter's decoder sees
/// it. An owned `String` is encode-only, but a borrowed `&[u8]` byte slice
/// decodes ZERO-COPY as a length-prefixed view of the buffer (`ByteSlice`).
enum WireInterpScalarField {
    Scalar {
        encoding: psi_typed_trees::wire::WireScalarEncoding,
        range: Option<psi_language_semantics::wire::WireScalarRange>,
    },
    Nested(
        Vec<(
            String,
            u64,
            psi_typed_trees::wire::WireScalarEncoding,
            Option<psi_language_semantics::wire::WireScalarRange>,
        )>,
    ),
    Repeated {
        encoding: psi_typed_trees::wire::WireRepeatedEncoding,
        range: Option<psi_language_semantics::wire::WireScalarRange>,
    },
    /// A borrowed `&[u8]` field: read a byte-length varint then that many bytes
    /// from the buffer. Stored as an owned `Array` of byte values --
    /// observationally identical to a zero-copy view for any read. The
    /// `predicates` are the slice's declared byte-domain obligations,
    /// evaluated over the UNTRUSTED wire bytes at the decode boundary.
    ByteSlice {
        predicates: Vec<psi_typed_trees::byte_predicates::ByteSequencePredicate>,
    },
}

/// The CURRENT-era (name, number, scalar encoding) list of a nested wire
/// schema, sorted by field number -- validation has already guaranteed the
/// scalar-only child body.
fn wire_nested_scalar_fields(
    program: &TypedTrees,
    child: &psi_typed_trees::wire::WireSchema,
) -> Result<Vec<(String, u64, psi_typed_trees::wire::WireScalarEncoding)>, Halt> {
    use psi_typed_trees::wire::{WireMember, WireScalarEncoding};

    let mut children = Vec::new();
    for member in program.wire_members(child.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let scalar = program
            .primitive_type_reference(field.type_reference)
            .and_then(WireScalarEncoding::for_primitive)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "data `{}` nested field `{}` is not a stage 2 scalar",
                    child.name, field.name
                ))
            })?;
        children.push((field.name.as_str().to_owned(), field.number, scalar));
    }
    children.sort_by_key(|(_, number, _)| *number);
    Ok(children)
}

/// Decode-side nested fields additionally carry the destination declaration's
/// range, because the schema primitive alone does not contain that fact.
fn wire_nested_decode_scalar_fields(
    program: &TypedTrees,
    child: &psi_typed_trees::wire::WireSchema,
    value_type: TypeReferenceHandle,
) -> Result<
    Vec<(
        String,
        u64,
        psi_typed_trees::wire::WireScalarEncoding,
        Option<psi_language_semantics::wire::WireScalarRange>,
    )>,
    Halt,
> {
    use psi_typed_trees::wire::{WireMember, WireScalarEncoding};

    let mut children = Vec::new();
    for member in program.wire_members(child.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let target_type =
            psi_typed_trees::wire::data_field_type(program, value_type, field.name.as_str())
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{}` nested destination has no field `{}`",
                        child.name, field.name
                    ))
                })?;
        let scalar = program
            .primitive_type_reference(field.type_reference)
            .and_then(WireScalarEncoding::for_primitive)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "data `{}` nested field `{}` is not a stage 2 scalar",
                    child.name, field.name
                ))
            })?;
        children.push((
            field.name.as_str().to_owned(),
            field.number,
            scalar,
            psi_typed_trees::wire::scalar_decode_range(program, target_type),
        ));
    }
    children.sort_by_key(|(_, number, _, _)| *number);
    Ok(children)
}

fn wire_argument_declared_type(
    program: &TypedTrees,
    frame: &Frame,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => wire_argument_declared_type(program, frame, inner.target),
        ExpressionNode::Member(member) => {
            let receiver = wire_argument_declared_type(program, frame, member.receiver)?;
            psi_typed_trees::wire::data_field_type(program, receiver, member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let mut current = *frame.type_locals.borrow().get(members.first()?.as_str())?;
            for member in members.iter().skip(1) {
                current =
                    psi_typed_trees::wire::data_field_type(program, current, member.as_str())?;
            }
            Some(current)
        }
        _ => None,
    }
}

fn wire_scalar_in_range(
    raw: u64,
    encoding: psi_typed_trees::wire::WireScalarEncoding,
    value: &Value,
    range: psi_language_semantics::wire::WireScalarRange,
) -> bool {
    if range.signed {
        let value = if encoding.zigzag {
            unzigzag64(raw)
        } else {
            let Some(value) = value.as_int() else {
                return false;
            };
            value
        };
        value >= range.minimum && value <= range.maximum
    } else {
        raw >= range.minimum as u64 && raw <= range.maximum as u64
    }
}

/// The unsigned LEB128 payload a scalar value encodes as -- the same
/// widths/signedness the native encoders apply: load at the source width
/// (zero- or sign-extending), zigzag signed sources at 64 bits.
fn wire_scalar_varint_value(
    raw: i64,
    scalar: psi_typed_trees::wire::WireScalarEncoding,
) -> Result<u64, Halt> {
    match (scalar.byte_size, scalar.zigzag) {
        (1, _) => Ok(u64::from(raw != 0)),
        (4, false) => Ok(u64::from(raw as u32)),
        (8, false) => Ok(raw as u64),
        (4, true) => Ok(zigzag64(i64::from(raw as i32))),
        (8, true) => Ok(zigzag64(raw)),
        _ => Err(Halt::Unsupported(format!(
            "wire scalar of {} bytes",
            scalar.byte_size
        ))),
    }
}

/// The decoded value a raw LEB128 payload produces -- the same
/// widths/signedness the native decoders apply: truncate to the field width,
/// un-zigzag signed targets at 64 bits first.
fn wire_decoded_scalar_value(
    raw: u64,
    encoding: psi_typed_trees::wire::WireScalarEncoding,
) -> Result<Value, Halt> {
    match (encoding.byte_size, encoding.zigzag) {
        (1, _) => Ok(Value::Bool((raw & 0xff) != 0)),
        (4, false) => Ok(Value::Int(i64::from(raw as u32))),
        (8, false) => Ok(Value::Int(raw as i64)),
        (4, true) => Ok(Value::Int(i64::from(unzigzag64(raw) as i32))),
        (8, true) => Ok(Value::Int(unzigzag64(raw))),
        _ => Err(Halt::Unsupported(format!(
            "wire scalar of {} bytes",
            encoding.byte_size
        ))),
    }
}

fn zigzag64(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// unzigzag(n) = (n >> 1) ^ -(n & 1): the signed-scalar post-step of the
/// compact_binary v0 varint decode, identical to the native decoders'
/// shift/mask/xor.
fn unzigzag64(value: u64) -> i64 {
    ((value >> 1) ^ (value & 1).wrapping_neg()) as i64
}

/// Inclusive [min, max] of an integer primitive as i64. `None` for widths whose
/// range cannot be represented in i64 (u64/usize) -- their saturating/trapping
/// behaviour is not modelled by the interpreter yet (they fall back to wrap).
fn integer_bounds(ty: PrimitiveType) -> Option<(i64, i64)> {
    match ty {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        _ => None,
    }
}

fn semantic_integer_format(ty: PrimitiveType) -> Option<SemanticIntegerFormat> {
    match ty {
        PrimitiveType::I8 => Some(SemanticIntegerFormat::I8),
        PrimitiveType::I16 => Some(SemanticIntegerFormat::I16),
        PrimitiveType::I32 => Some(SemanticIntegerFormat::I32),
        PrimitiveType::I64 => Some(SemanticIntegerFormat::I64),
        PrimitiveType::U8 => Some(SemanticIntegerFormat::U8),
        PrimitiveType::U16 => Some(SemanticIntegerFormat::U16),
        PrimitiveType::U32 => Some(SemanticIntegerFormat::U32),
        PrimitiveType::U64 | PrimitiveType::Addr => Some(SemanticIntegerFormat::U64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

fn is_unsigned_integer_primitive(ty: PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr
    )
}

fn big_integer_runtime_value(value: &BigInt, ty: PrimitiveType) -> i64 {
    if is_unsigned_integer_primitive(ty) {
        value
            .to_u64()
            .expect("checked unsigned conversion fits its target") as i64
    } else {
        value
            .to_i64()
            .expect("checked signed conversion fits its target")
    }
}

fn float_to_integer_trap_message(
    target: PrimitiveType,
    reason: FloatToIntegerError,
    trapping: bool,
) -> String {
    let operation = if trapping { "Trapping" } else { "Exact" };
    let reason = match reason {
        FloatToIntegerError::NonFinite => "the value is not finite",
        FloatToIntegerError::OutOfRange => "the truncated value is out of range",
    };
    format!("float-to-int conversion failed in {operation} domain: {reason} for {target:?}")
}

/// Preserve an f32 NaN's sign and payload while carrying interpreter floats in
/// the existing f64-backed `Value::Float`. Finite values and infinities remain
/// ordinary numeric f64 values; only NaNs use this reversible payload embedding.
fn interpreter_f32_from_bits(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return value as f64;
    }

    let sign = ((bits as u64) >> 31) << 63;
    let payload = (u64::from(bits) & 0x007f_ffff) << 29;
    f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
}

fn interpreter_f32_to_bits(value: f64) -> u32 {
    if !value.is_nan() {
        return (value as f32).to_bits();
    }

    let bits = value.to_bits();
    let sign = ((bits >> 63) as u32) << 31;
    let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
    payload &= 0x007f_ffff;
    if payload == 0 {
        payload = 0x0040_0000;
    }
    sign | 0x7f80_0000 | payload
}

/// Apply a write target's arithmetic domain (decision 17) to a raw i64 result,
/// mirroring the native backend so the differential oracle agrees:
/// Exact/Wrapping truncate to width; Saturating clamps to [min, max]; Trapping
/// halts (overflow trap) when the value is out of range.
fn apply_arithmetic_domain(
    raw: i64,
    ty: PrimitiveType,
    domain: ArithmeticDomain,
) -> EvalResult<i64> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => Ok(wrap_to_width(raw, ty)),
        ArithmeticDomain::Saturating => match integer_bounds(ty) {
            Some((min, max)) => Ok(raw.clamp(min, max)),
            None => Ok(wrap_to_width(raw, ty)),
        },
        ArithmeticDomain::Trapping => match integer_bounds(ty) {
            Some((min, max)) if raw < min || raw > max => trap(format!(
                "arithmetic overflow in Trapping domain: {raw} is out of range for {ty:?}"
            )),
            _ => Ok(wrap_to_width(raw, ty)),
        },
    }
}

/// The bit width a WRAPPING shift wraps at (the modular-arithmetic modulus
/// exponent). Pointer-width types are 64-bit in both engines.
fn primitive_bit_width(ty: PrimitiveType) -> u64 {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => 8,
        PrimitiveType::I16 | PrimitiveType::U16 => 16,
        PrimitiveType::I32 | PrimitiveType::U32 => 32,
        _ => 64,
    }
}

fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
    match ty {
        PrimitiveType::I8 => raw as i8 as i64,
        PrimitiveType::U8 => raw as u8 as i64,
        PrimitiveType::I16 => raw as i16 as i64,
        PrimitiveType::U16 => raw as u16 as i64,
        PrimitiveType::I32 => raw as i32 as i64,
        PrimitiveType::U32 => raw as u32 as i64,
        // 64-bit and pointer-width types keep the full value (unsigned reinterpretation of a
        // u64 is still represented by the same bit pattern in i64).
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => raw,
        // Non-integer primitives do not reach this path.
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => raw,
    }
}

/// What a satisfied transition decided to do next.
#[derive(Clone)]
struct EvaluatedArgument {
    cell: Cell,
    mutable_recast: Option<MutableScalarRecast>,
}

impl EvaluatedArgument {
    fn plain(cell: Cell) -> Self {
        Self {
            cell,
            mutable_recast: None,
        }
    }
}

enum TransitionDecision {
    Terminal,
    SelfTarget,
    Value(Value),
    Named {
        state_name: String,
        machine: Machine,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    },
}

// `Frame::locals` needs interior mutability so `let` bindings can be added while the
// frame is shared by `&`. Wrap the map in a RefCell.
/// Byte width of an integer primitive -- the PROMOTION rank a mixed-width
/// binary node computes in. `None` for non-integer primitives.
fn integer_primitive_byte_width(ty: PrimitiveType) -> Option<usize> {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => Some(8),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

fn primitive_is_unsigned64(primitive: Option<PrimitiveType>) -> bool {
    matches!(primitive, Some(PrimitiveType::U64 | PrimitiveType::Addr))
}

impl Frame {
    fn get(&self, name: &str) -> Option<Cell> {
        self.locals_ref().borrow().get(name).cloned()
    }

    fn bind(&self, name: &str, cell: Cell) {
        self.locals_ref().borrow_mut().insert(name.to_owned(), cell);
    }

    fn bind_type(&self, name: &str, type_reference: TypeReferenceHandle) {
        self.type_locals
            .borrow_mut()
            .insert(name.to_owned(), type_reference);
    }

    fn locals_ref(&self) -> &RefCell<BTreeMap<String, Cell>> {
        &self.locals
    }
}
