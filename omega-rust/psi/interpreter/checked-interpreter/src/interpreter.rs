//! Checked execution lifecycle: choose authority, run a scoped worker, retain outcomes.
//!
//! Full-program execution starts at `interpret_entry`, keyed by
//! [`BuildMachineEntry`]. The remaining early jobs return pure values.
//! Build activations and their mutable argument custody run through Terminal Psi.

mod evaluator;
use crate::{
    BuildTimeOperationEvaluation, EvaluationUsage, FilesystemAccess, FilesystemMetadataLayout,
    InterpretOutcome, MeasuredEvaluation, SelectedBuildTimeBinaryOperator,
};
use build_time_values::BuildTimeValue;
use checked_trees::CheckedTrees;
use evaluator::{CONST_EVAL_STEP_BUDGET, Evaluator, Halt, ambient_step_budget};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Options for [`interpret_entry`]. `Default` selects the hermetic
/// virtual filesystem and the compiler host's checked standard metadata
/// carrier. Cross-target and package-build callers supply the selected checked
/// metadata layout explicitly.
#[derive(Clone, Debug, Default)]
pub struct InterpretOptions<'bindings> {
    /// Explicit Product execution associations, separate from immutable Psi.
    pub boundary_adapters: &'bindings [checked_trees::CheckedBoundaryAdapterDispatch],
    pub float_comparisons: &'bindings [checked_trees::CheckedSelectedFloatComparisonExecution],
    /// The realization owner's supplied x86 processor flags image, captured
    /// from the executing host. `pushfq` observes this environment and `popfq`
    /// restores its arithmetic bits; both decline without it rather than
    /// fabricating an image.
    pub x86_flags: Option<u64>,
    /// Exact externally selected wait requirements, served only by virtual time.
    pub virtual_clock_bindings: Vec<crate::VirtualClockBinding>,
    pub filesystem: FilesystemAccess,
    pub filesystem_metadata_layout: FilesystemMetadataLayout,
}

/// Interpret a checked program from one exact machine identity.
///
/// Build/target selection owns this identity. The interpreter neither discovers
/// an entry from source spelling nor retries alternate names. A `Build::bind`
/// selection already resolved its machine to a symbol under the binding
/// occurrence's lexical package; [`BuildMachineEntry::Symbol`] consumes that
/// identity directly so a same-named declaration elsewhere in the program
/// cannot be picked up instead, while [`BuildMachineEntry::Name`] is the
/// authored-name lookup for ad hoc callers. `stdin` provides the bytes a
/// `read_line` host call would consume.
pub fn interpret_entry(
    checked: &CheckedTrees,
    entry: BuildMachineEntry<'_>,
    stdin: &[u8],
    options: InterpretOptions<'_>,
) -> InterpretOutcome {
    // Run on a worker thread with a generous stack: the tree-walker recurses with the
    // program's call/expression nesting, which can exceed the default test-thread stack on
    // deep programs even with the call-depth budget. A scoped thread lets us keep the
    // borrow of `checked`/`stdin`.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                interpret_on_current_thread(checked, entry, stdin, options)
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
///
/// This is a pure value request: it carries no host grants, sponsor, or product
/// selection authority. Build activation has its own Terminal host.
pub struct BuildMachineEvaluationRequest<'a> {
    pub entry: BuildMachineEntry<'a>,
    pub arguments: Vec<BuildTimeValue>,
    pub operators: &'a [SelectedBuildTimeBinaryOperator],
}

/// The immutable source and facts available to a compiler invocation. Early
/// elaboration can retain typed source; a checked constant job or Build activation
/// retains the checker's exact arena and operator execution facts. Neither variant
/// imports a runtime provider's boundary dispatch or host-service authority.
#[derive(Clone, Copy)]
pub enum BuildMachineProgram<'program> {
    Typed(&'program TypedTrees),
    Checked(&'program checked_trees::CheckedTrees),
    /// Checked executable closure and a separate, immutable product declaration
    /// frontier. Only designated product-description operations read the latter;
    /// ordinary machine lookup remains confined to `execution`.
    CheckedClosure {
        execution: &'program checked_trees::CheckedTrees,
        product_declarations: &'program TypedTrees,
    },
}

impl<'program> BuildMachineProgram<'program> {
    fn typed(self) -> &'program TypedTrees {
        match self {
            Self::Typed(typed) => typed,
            Self::Checked(checked) => &checked.typed,
            Self::CheckedClosure { execution, .. } => &execution.typed,
        }
    }
}

impl<'a> BuildMachineEvaluationRequest<'a> {
    /// Evaluate the machine with this authored name, unsponsored, with no
    /// selected operators.
    pub fn named(machine_name: &'a str, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Name(machine_name),
            arguments,
            operators: &[],
        }
    }

    /// Evaluate the machine behind this symbol, unsponsored, with no selected
    /// operators.
    pub fn symbol(machine_symbol: SymbolHandle, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Symbol(machine_symbol),
            arguments,
            operators: &[],
        }
    }
}

/// Materialize the interpreter's ordinary zero representation for a compiler-
/// supplied argument. The caller must establish that zero inhabits this exact
/// declared type before requesting it; this operation grants no domain fact or
/// reference authority. Policy scheduling uses it for fresh, private context
/// storage, then passes that value through the ordinary positional evaluator.
pub fn materialize_zero_initialized_build_time_value(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<BuildTimeValue, String> {
    let mut evaluator = Evaluator::new(program, &[]);
    let value =
        evaluator
            .default_value_for_type(type_reference)
            .map_err(|reason| match reason {
                Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => {
                    message
                }
                Halt::Exit(code) => format!("zero storage initialization exited with code {code}"),
                Halt::Crash(cause, site) => format!("explicit crash {cause:?} at {site}"),
            })?;
    crate::build_time::observe_build_time_value(&value, &|reference| {
        evaluator.read_reference(reference)
    })
    .map_err(|reason| match reason {
        Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => message,
        Halt::Exit(code) => format!("zero storage observation exited with code {code}"),
        Halt::Crash(cause, site) => format!("explicit crash {cause:?} at {site}"),
    })
}

/// Current fixed step ceiling for pure source evaluation. Retained invocation
/// charges compare this policy as well as their schema and step schedule before
/// replacing work; the value is the same one fresh evaluation configures.
pub const fn pure_evaluation_step_ceiling() -> u64 {
    CONST_EVAL_STEP_BUDGET
}

/// Evaluate one build-time machine for its structured return value and the
/// private layout placements it issued. Selected operators are honored; the
/// evaluation is pure and unsponsored.
pub fn evaluate_build_time_machine(
    program: &TypedTrees,
    request: BuildMachineEvaluationRequest<'_>,
) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, String> {
    evaluate_prepared_build_time_machine(BuildMachineProgram::Typed(program), request)
}

/// Ordinary early-job preparation retains its checked facts when returning a
/// value, just as Build activation does when observing argument storage. Both
/// entrances use this existing evaluator; Terminal-only execution will replace
/// the evaluator after its checker and admission prerequisites are complete.
pub fn evaluate_prepared_build_time_machine(
    program: BuildMachineProgram<'_>,
    request: BuildMachineEvaluationRequest<'_>,
) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, String> {
    evaluate_structured_return(program, request.entry, request.arguments, request.operators)
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
        Err(halt) => Err(value_return_halt_error(halt)),
    }
}

/// The value-returning evaluation's halt text: an explicit exit reports its
/// code against the value expectation; every other halt carries its message.
fn value_return_halt_error(halt: Halt) -> String {
    match halt {
        Halt::Exit(code) => format!(
            "the machine attempted to exit the process (code {code}) instead of returning a value"
        ),
        Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => message,
        Halt::Crash(cause, site) => format!("explicit crash {cause:?} at {site}"),
    }
}

fn evaluate_structured_return(
    program: BuildMachineProgram<'_>,
    entry: BuildMachineEntry<'_>,
    arguments: Vec<build_time_values::BuildTimeValue>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
) -> Result<BuildTimeOperationEvaluation<build_time_values::BuildTimeValue>, String> {
    let typed = program.typed();
    if operators
        .iter()
        .any(|operator| operator.has_crash_contract(typed))
    {
        return Err(
            "selected operator crash invocations have no build-time execution support".into(),
        );
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new_build(program);
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
                            build_time_result_custody(std::slice::from_ref(&value)).ok_or_else(
                                || "build-time evaluator result-cell count overflowed".to_owned(),
                            )?;
                        usage.record_result_custody(result_cells, result_text_bytes);
                        Ok(BuildTimeOperationEvaluation::new(
                            value,
                            usage,
                            evaluator.private_layout_placements,
                        ))
                    }
                    Err(halt) => Err(value_return_halt_error(halt)),
                }
            })
            .expect("spawn build-time evaluation worker thread")
            .join()
            .unwrap_or_else(|_| Err("build-time evaluator thread panicked".to_owned()))
    })
}

fn interpret_on_current_thread(
    checked: &CheckedTrees,
    entry: BuildMachineEntry<'_>,
    stdin: &[u8],
    options: InterpretOptions<'_>,
) -> InterpretOutcome {
    let mut evaluator = Evaluator::new_checked(checked, stdin);
    evaluator.boundary_adapter_dispatch = options.boundary_adapters;
    evaluator.selected_product_float_comparisons = Some(options.float_comparisons);
    evaluator.x86_flags = match options.x86_flags {
        Some(image) => match language_core::inline_assembly::X86FlagsEnvironment::new(image) {
            Ok(environment) => Some(environment),
            Err(error) => return evaluator.error_outcome(error.to_string()),
        },
        None => None,
    };
    if checked
        .facts
        .operators
        .has_crash_qualified_uses(&checked.typed)
    {
        return evaluator.error_outcome(
            "selected operator crash invocations have no checked execution support".to_owned(),
        );
    }
    for (position, binding) in options.virtual_clock_bindings.iter().enumerate() {
        let validity = binding.validate_for(checked).and_then(|()| {
            if options.virtual_clock_bindings[..position]
                .iter()
                .any(|earlier| earlier.overlaps(binding))
            {
                Err("virtual wait requirement has duplicate bindings")
            } else {
                Ok(())
            }
        });
        if let Err(message) = validity {
            return evaluator.error_outcome(message.to_owned());
        }
    }
    evaluator.virtual_clock_bindings = options.virtual_clock_bindings;
    evaluator.filesystem_metadata_layout = options.filesystem_metadata_layout;
    match options.filesystem {
        FilesystemAccess::Virtual => {}
        FilesystemAccess::RealUnscoped => {
            evaluator.real_fs = Some(
                crate::interpreter::evaluator::filesystem::real::RealFs::new(None, None)
                    .expect("unscoped filesystem has no grant configuration"),
            );
        }
        FilesystemAccess::RealScoped(grants) => {
            let filesystem = match crate::interpreter::evaluator::filesystem::real::RealFs::new(
                Some(grants),
                None,
            ) {
                Ok(filesystem) => filesystem,
                Err(message) => {
                    return evaluator.error_outcome(message.to_owned());
                }
            };
            evaluator.real_fs = Some(filesystem);
        }
        FilesystemAccess::CapturedScoped {
            inputs,
            grants,
            sponsor,
        } => {
            match crate::interpreter::evaluator::filesystem::real::RealFs::with_captured_inputs(
                grants, sponsor, inputs,
            ) {
                Ok(filesystem) => evaluator.real_fs = Some(filesystem),
                Err(message) => {
                    return evaluator.error_outcome(message.to_owned());
                }
            }
        }
        FilesystemAccess::RealScopedSponsored { grants, sponsor } => {
            let filesystem = match crate::interpreter::evaluator::filesystem::real::RealFs::new(
                Some(grants),
                Some(sponsor),
            ) {
                Ok(filesystem) => filesystem,
                Err(message) => {
                    return evaluator.error_outcome(message.to_owned());
                }
            };
            evaluator.real_fs = Some(filesystem);
        }
    }
    let result = match entry {
        BuildMachineEntry::Name(entry_machine_name) => evaluator.run_entry(entry_machine_name),
        BuildMachineEntry::Symbol(entry_machine_symbol) => {
            evaluator.run_entry_symbol(entry_machine_symbol)
        }
    };
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
        Err(Halt::Crash(cause, site)) => {
            InterpretOutcome::crashed(cause, site, evaluator.stdout, evaluator.stderr, usage)
        }
        Err(Halt::Unsupported(message))
        | Err(Halt::Trap(message))
        | Err(Halt::Resource(message)) => {
            InterpretOutcome::error(message, evaluator.stdout, evaluator.stderr, usage)
        }
    }
}

fn build_time_result_custody(values: &[build_time_values::BuildTimeValue]) -> Option<(u64, u64)> {
    values
        .iter()
        .try_fold((0u64, 0u64), |(cells, text_bytes), value| {
            Some((
                cells.checked_add(value.retained_cell_count()?)?,
                text_bytes.checked_add(value.retained_text_byte_count()?)?,
            ))
        })
}
