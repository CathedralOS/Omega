//! Checked execution lifecycle: choose authority, run a scoped worker, retain outcomes.
//!
//! Full-program execution starts at `interpret_entry`. Pure returned
//! values, observed final arguments, and filesystem-granted arguments keep separate
//! execution paths because their authority and failure evidence differ.

mod evaluator;
use crate::{
    BuildEvaluationSponsor, BuildMachineEvaluationFailure, BuildMachineEvaluationFailureKind,
    BuildTimeOperationEvaluation, BuildTimeValue, EvaluationObservations, EvaluationUsage,
    FilesystemAccess, FilesystemMetadataLayout, FilesystemServiceBinding, InterpretOutcome,
    MeasuredBuildMachineEvaluation, MeasuredEvaluation, SelectedBuildTimeBinaryOperator,
};
use checked_trees::CheckedTrees;
use evaluator::{
    CONST_EVAL_STEP_BUDGET, Evaluator, Halt, STEP_BUDGET, ambient_step_budget, real_filesystem,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Interpret a checked program from one exact machine identity.
///
/// Build/target selection owns this identity. The interpreter neither discovers
/// an entry from source spelling nor retries alternate names. `stdin` provides
/// the bytes a `read_line` host call would consume.
/// Options for [`interpret_entry`]. `Default` selects the hermetic
/// virtual filesystem and the compiler host's checked standard metadata
/// carrier. Cross-target and package-build callers supply the selected checked
/// metadata layout explicitly.
#[derive(Clone, Debug, Default)]
pub struct InterpretOptions {
    pub filesystem: FilesystemAccess,
    pub filesystem_metadata_layout: FilesystemMetadataLayout,
    pub filesystem_service_binding: Option<FilesystemServiceBinding>,
}

impl InterpretOptions {
    #[must_use]
    pub fn with_filesystem_service_binding(mut self, binding: FilesystemServiceBinding) -> Self {
        self.filesystem_service_binding = Some(binding);
        self
    }
}

/// [`interpret_entry`] with explicit [`InterpretOptions`].
pub fn interpret_entry(
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
                interpret_on_current_thread(checked, entry_machine_name, stdin, options)
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

/// CONST EVALUATION (comptime stage 1): evaluate the zero-argument machine
/// `machine_name` at compile time, returning its terminal value width-adjusted
/// to the machine's declared integer return type (TARGET widths -- the same
/// wrap-on-write the differential interpreter applies -- never host widths).
///
/// The CALLER owns the legality gate (the machine's inferred transitive effect
/// surface must be empty and it must take no parameters -- frozen decision 12's
/// purity predicate); this entry owns evaluation only. Termination rides the
/// language's existing discipline (no general recursion, loops carry
/// decreases); a small evaluator-step ceiling (~100k units) backstops checker
/// gaps. Errors are human-readable reasons for the compile diagnostic at the
/// const site.
///
/// Works over `TypedTrees` (pre-checking) so the compiler pipeline can
/// substitute results BEFORE range checking and layout consume the lengths.
pub fn evaluate_const_machine(
    program: &typed_trees::TypedTrees,
    machine_name: &str,
) -> Result<i64, String> {
    evaluate_const_machine_measured(program, machine_name).map(MeasuredEvaluation::into_value)
}

/// [`evaluate_const_machine`] with its deterministic evaluator usage.
pub fn evaluate_const_machine_measured(
    program: &typed_trees::TypedTrees,
    machine_name: &str,
) -> Result<MeasuredEvaluation<i64>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                evaluate_const_on_current_thread(program, machine_name)
            })
            .expect("spawn const-eval worker thread")
            .join()
            .unwrap_or_else(|_| Err("const evaluator thread panicked".to_owned()))
    })
}

// STRUCTURED build-time evaluation (wiki/spec/language/evaluation.md;
// the R2 layouts enabler): invoke the effect-free machine `machine_name`
// with compiler-built arguments and read back its terminal value as a
// structured tree. As with [`evaluate_const_machine`], the CALLER owns the
// legality gate (decision 12's transitive effect surface must be empty and
// parameters must be by-value); this entry owns evaluation, positional
// argument binding (count-checked), and the evaluator-step ceiling. No keyword
// marks build-time machines -- the position makes the evaluation build-time,
// and the effect system makes it legal.

/// Which build-time machine an evaluation runs.
#[derive(Clone, Copy, Debug)]
pub enum BuildMachineEntry<'a> {
    /// The machine with this authored name.
    Name(&'a str),
    /// The machine behind this resolved symbol.
    Symbol(SymbolHandle),
}

/// One build-time machine evaluation: the machine, its arguments, the selected
/// build-time operators a structured evaluation may call, and the sponsor an
/// argument evaluation charges for its result custody.
pub struct BuildMachineEvaluationRequest<'a> {
    pub entry: BuildMachineEntry<'a>,
    pub arguments: Vec<BuildTimeValue>,
    pub operators: &'a [SelectedBuildTimeBinaryOperator],
    pub sponsor: Option<&'a BuildEvaluationSponsor>,
}

impl<'a> BuildMachineEvaluationRequest<'a> {
    /// Evaluate the machine with this authored name, unsponsored, with no
    /// selected operators.
    pub fn named(machine_name: &'a str, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Name(machine_name),
            arguments,
            operators: &[],
            sponsor: None,
        }
    }

    /// Evaluate the machine behind this symbol, unsponsored, with no selected
    /// operators.
    pub fn symbol(machine_symbol: SymbolHandle, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Symbol(machine_symbol),
            arguments,
            operators: &[],
            sponsor: None,
        }
    }
}

/// Evaluate one build-time machine for its structured return value and the
/// private layout placements it issued. Selected operators are honored; the
/// evaluation is pure and unsponsored.
pub fn evaluate_build_time_machine(
    program: &TypedTrees,
    request: BuildMachineEvaluationRequest<'_>,
) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, String> {
    evaluate_structured_return(program, request.entry, request.arguments, request.operators)
}

/// Evaluate one build-time machine for its observed argument values without
/// a filesystem grant, charging the sponsor when one is given.
pub fn evaluate_build_machine_arguments(
    program: &TypedTrees,
    request: BuildMachineEvaluationRequest<'_>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, String> {
    evaluate_observed_arguments(
        program,
        request.entry,
        request.arguments,
        request.sponsor.cloned(),
    )
}

/// Evaluate one build-time machine for its observed argument values under
/// the filesystem grant in `options`, charging the sponsor when one is given.
pub fn evaluate_granted_build_machine_arguments(
    program: &TypedTrees,
    request: BuildMachineEvaluationRequest<'_>,
    options: InterpretOptions,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationFailure> {
    evaluate_granted_arguments(
        program,
        request.entry,
        request.arguments,
        options,
        request.sponsor.cloned(),
    )
}

fn evaluate_const_on_current_thread(
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

fn evaluate_structured_return(
    program: &TypedTrees,
    entry: BuildMachineEntry<'_>,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
) -> Result<BuildTimeOperationEvaluation<crate::build_time::BuildTimeValue>, String> {
    if operators
        .iter()
        .any(|operator| operator.has_crash_contract(program))
    {
        return Err(
            "selected operator crash invocations have no build-time execution support".into(),
        );
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.selected_build_time_operators = operators;
                evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, None);
                let result = match entry {
                    BuildMachineEntry::Name(machine_name) => {
                        evaluator.run_build_time_machine(machine_name, arguments)
                    }
                    BuildMachineEntry::Symbol(machine_symbol) => {
                        evaluator.run_build_time_machine_symbol(machine_symbol, arguments)
                    }
                };
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

fn evaluate_observed_arguments(
    program: &TypedTrees,
    entry: BuildMachineEntry<'_>,
    arguments: Vec<crate::build_time::BuildTimeValue>,
    sponsor: Option<BuildEvaluationSponsor>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<crate::build_time::BuildTimeValue>>, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new(program, &[]);
                evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, sponsor);
                let result = match entry {
                    BuildMachineEntry::Name(machine_name) => {
                        evaluator.run_build_time_machine_arguments(machine_name, arguments)
                    }
                    BuildMachineEntry::Symbol(machine_symbol) => evaluator
                        .run_build_time_machine_symbol_arguments(machine_symbol, arguments),
                };
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
                    std::mem::take(&mut evaluator.output_obligations),
                    std::mem::take(&mut evaluator.output_receipts),
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
                            std::mem::take(&mut evaluator.executed_root_bindings),
                            std::mem::take(&mut evaluator.executed_behavior_exclusions),
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

fn evaluate_granted_arguments(
    program: &TypedTrees,
    entry: BuildMachineEntry<'_>,
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
                if options.filesystem_service_binding.is_some() {
                    return Err(BuildMachineEvaluationFailure::without_evidence(
                        BuildMachineEvaluationFailureKind::Unsupported,
                        "a checked-program filesystem service binding cannot enter typed build evaluation".to_owned(),
                    ));
                }
                let replaying = matches!(
                    &options.filesystem,
                    FilesystemAccess::ReplayFilesystem(_)
                );
                match options.filesystem {
                    FilesystemAccess::Virtual => {}
                    FilesystemAccess::RealUnscoped => {
                        evaluator.real_fs = Some(
                            real_filesystem::RealFs::new(None, None)
                                .expect("unscoped filesystem has no grant configuration"),
                        );
                    }
                    FilesystemAccess::RealScoped(grants) => {
                        evaluator.real_fs = Some(real_filesystem::RealFs::new(Some(grants), None).map_err(
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
                            real_filesystem::RealFs::new(Some(grants), Some(sponsor)).map_err(
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
                let result = match entry {
                    BuildMachineEntry::Name(machine_name) => evaluator
                        .run_build_machine_arguments_with_policy(machine_name, arguments, true),
                    BuildMachineEntry::Symbol(machine_symbol) => evaluator
                        .run_build_machine_symbol_arguments_with_policy(
                            machine_symbol,
                            arguments,
                            true,
                        ),
                }
                .and_then(|values| {
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
                    std::mem::take(&mut evaluator.output_obligations),
                    std::mem::take(&mut evaluator.output_receipts),
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
                            std::mem::take(&mut evaluator.executed_root_bindings),
                            std::mem::take(&mut evaluator.executed_behavior_exclusions),
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

fn interpret_on_current_thread(
    checked: &CheckedTrees,
    entry_machine_name: &str,
    stdin: &[u8],
    options: InterpretOptions,
) -> InterpretOutcome {
    let mut evaluator = Evaluator::new_checked(checked, stdin);
    if checked
        .facts
        .operators
        .has_crash_qualified_uses(&checked.typed)
    {
        return InterpretOutcome::error(
            "selected operator crash invocations have no checked execution support".to_owned(),
            evaluator.stdout,
            evaluator.stderr,
            evaluator.usage,
        );
    }
    if checked
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(
                expression,
                checked_trees::expression::ExpressionNode::Atomic(atomic)
                    if matches!(
                        atomic.ordering,
                        language_core::AtomicOrderingPlan::CompareExchangeOnce { .. }
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
    evaluator.filesystem_service_symbol = match options.filesystem_service_binding {
        Some(binding) => match binding.declaration_symbol_for(checked) {
            Ok(symbol) => Some(symbol),
            Err(message) => {
                return InterpretOutcome::error(
                    message,
                    evaluator.stdout,
                    evaluator.stderr,
                    evaluator.usage,
                );
            }
        },
        None => None,
    };
    match options.filesystem {
        FilesystemAccess::Virtual => {}
        FilesystemAccess::RealUnscoped => {
            evaluator.real_fs = Some(
                real_filesystem::RealFs::new(None, None)
                    .expect("unscoped filesystem has no grant configuration"),
            );
        }
        FilesystemAccess::RealScoped(grants) => {
            let filesystem = match real_filesystem::RealFs::new(Some(grants), None) {
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
            let filesystem = match real_filesystem::RealFs::new(Some(grants), Some(sponsor)) {
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
    use super::interpret_on_current_thread;
    use crate::InterpretOptions;
    use checked_trees::CheckedTrees;
    use checked_trees::expression::{ExpressionNode, TableAtomicExpression};
    use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
    use numerics::literals::IntegerLiteral;

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
            result_custody:
                language_core::atomic::AtomicExpressionResultCustody::ObservingCompareExchangeOnce(
                    language_core::atomic::AtomicCompareExchangeOnceResultCustody::CANONICAL,
                ),
        }));

        let outcome =
            interpret_on_current_thread(&checked, "Main.main", &[], InterpretOptions::default());

        assert_eq!(
            outcome.error.as_deref(),
            Some("observing single-attempt compare-exchange has no runtime result carrier")
        );
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
