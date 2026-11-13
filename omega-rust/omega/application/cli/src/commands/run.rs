/*

Console adapter and exit-code policy for `omega run`.

We run a program in one of two shapes:
- Single run (`run_project`): compiles the root program once and executes it
  natively (`ExecutionSelection::Native`), in the Terminal Psi interpreter
  (`ExecutionSelection::Terminal`), or in both engines with differential
  comparison (`ExecutionSelection::Both`). When `--target` names a foreign
  target without `--engine terminal`, compilation stops after producing the
  target artifact (`ExecutionOutcome::TargetOnly`).
- Case batch (`run_cases`, selected when `--outputs` is present): compiles the
  program once on the host and runs every `*.stdin` file in `--inputs` (sorted
  lexicographically) or each explicit `--input` file in argument order. Per-case
  streams and `results.tsv` are written to `--outputs`.

Program stdout and stderr are raw byte streams, not UTF-8 text. In
`forward_output_or_exit`, we write and explicitly flush both streams before
printing any CLI status lines on stderr. Explicit flushes surface a broken pipe
before we choose an exit code; if forwarding fails, we make a best-effort write
to stderr (which may itself be the broken pipe) and exit 200 so a truncated
stream never masquerades as a clean program exit.

When `--engine terminal` fails to compile, no native compilation was requested,
so we prefix the diagnostics with `terminal compile FAILED:`. Native and
`--both` runs print `RunError` directly so sample-gate harnesses match the
established native failure wording.

*/

/*

Why `omega run` uses exit codes 200..=203 for harness and interpreter outcomes:

A guest program can legitimately exit with small positive statuses (including
1, 2, or 3). To keep `omega run` statuses unambiguous when a caller inspects
`$?`, we reserve 200..=203 for outcomes that are not a normal guest exit code:
- `200`: compile rejection, launch failure, output-forwarding failure, empty or
  unreadable `--inputs`, or Terminal Psi execution stopping without a process
  exit (unavailable host capability, sponsor work exhaustion, or pending host
  effect).
- `201`: differential divergence under `--both` (exit code mismatch, crash vs.
  normal exit, or Terminal Psi interpretation failure after native execution).
- `202`: compilation halted at `CompilerRequestOutcome::Incomplete` (a private
  provision bound was exceeded without refuting the source).
- `203`: guest crash inside `--engine terminal`. Unlike a native child process,
  the interpreter cannot present a guest trap as an OS termination signal
  without raising that signal on `omega` itself, so it reports the crash on
  stderr and exits 203.

*/

use crate::arguments::RunArguments;
use crate::execution::{
    CaseRunRequest, ExecutionOutcome, ExecutionSelection, InterpreterComparison, RunError,
    RunRequest, TerminalOutcome, run_project, run_project_cases,
};
use compiler::CompilerRequestOutcome;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn run(arguments: RunArguments) -> ! {
    if let Some(outputs) = arguments.outputs.clone() {
        run_cases(arguments, outputs);
    }
    let selection = arguments.selection;
    let outcome = match run_project(RunRequest {
        build_snapshot: arguments
            .build_inputs
            .map(|inputs| compiler::BuildSnapshotRequest::scoped(Vec::new(), inputs)),
        private_entry_ram: arguments.private_entry_ram,
        root_path: arguments.main_path,
        target_name: arguments.target_name,
        selection: arguments.selection,
        psi_work_budget: arguments.psi_work_budget,
        keep_artifacts: arguments.keep,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            match (&error, selection) {
                (RunError::Compilation(diagnostics), ExecutionSelection::Terminal) => {
                    eprintln!("terminal compile FAILED:");
                    for diagnostic in diagnostics {
                        eprintln!("  {diagnostic}");
                    }
                }
                _ => eprintln!("{error}"),
            }
            std::process::exit(compile_failure_status(&error));
        }
    };
    for entry in &outcome.test_roster {
        eprintln!("{entry}");
    }
    let status = match outcome.execution {
        ExecutionOutcome::Terminal(terminal) => present_terminal(terminal),
        ExecutionOutcome::TargetOnly { target_name } => {
            eprintln!(
                "compiled for target `{target_name}` OK ({})",
                outcome.build_dir.display()
            );
            0
        }
        ExecutionOutcome::Host {
            output,
            exit,
            comparison,
            private_entry_ram,
        } => {
            forward_output_or_exit(&output.stdout, &output.stderr);
            print_private_entry_ram(&private_entry_ram);
            let native_code = exit.code().unwrap_or(-1);
            eprintln!("native {}", exit.describe());
            match comparison {
                InterpreterComparison::NotRequested => native_code,
                InterpreterComparison::Declined(reason) => {
                    eprintln!("interp: DECLINED ({reason})");
                    native_code
                }
                InterpreterComparison::Agrees { exit_code } => {
                    eprintln!("interp exit: {exit_code}");
                    native_code
                }
                InterpreterComparison::AgreesCrash { cause } => {
                    eprintln!("interp crash: {cause:?} (host presentation agrees)");
                    native_code
                }
                InterpreterComparison::DisagreesCrash { cause } => {
                    eprintln!("interp crash: {cause:?}");
                    eprintln!(
                        "DIVERGENCE: native {} vs interp crash {cause:?}",
                        exit.describe()
                    );
                    201
                }
                InterpreterComparison::Disagrees {
                    interpreter_exit_code,
                } => {
                    eprintln!("interp exit: {interpreter_exit_code}");
                    eprintln!("DIVERGENCE: native {native_code} vs interp {interpreter_exit_code}");
                    201
                }
                InterpreterComparison::Failed(diagnostics) => {
                    eprintln!("Terminal interpretation FAILED:");
                    for diagnostic in diagnostics {
                        eprintln!("  {diagnostic}");
                    }
                    201
                }
            }
        }
    };
    if arguments.keep {
        eprintln!("build dir kept: {}", outcome.build_dir.display());
    }
    std::process::exit(status);
}

fn present_terminal(outcome: TerminalOutcome) -> i32 {
    use terminal_runtime::{TerminalExecutionResult, TerminalExecutionStatus};
    let observation = match outcome {
        TerminalOutcome::Completed(observation) => observation,
        TerminalOutcome::Unavailable(reason) => {
            eprintln!("terminal: UNAVAILABLE ({reason})");
            return 200;
        }
        TerminalOutcome::Failed(reason) => {
            eprintln!("Terminal execution FAILED: {reason}");
            return 200;
        }
    };
    forward_output_or_exit(&observation.stdout, &observation.stderr);
    print_private_entry_ram(&observation.private_entry_ram);
    match observation.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
            eprintln!("terminal exit 0");
            0
        }
        TerminalExecutionStatus::ExternalCompletion(completion) => {
            eprintln!("terminal exit {}", completion.return_code);
            completion.return_code
        }
        TerminalExecutionStatus::Crashed(crash) => {
            eprintln!("terminal crash: {:?}", crash.cause);
            203
        }
        TerminalExecutionStatus::Complete(result) => {
            eprintln!("terminal: program entry returned a non-process result: {result:?}");
            200
        }
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            eprintln!("psi: {exhaustion}; raise --psi-work-budget to permit more work");
            200
        }
        TerminalExecutionStatus::AwaitingEffect(site) => {
            eprintln!("terminal: execution awaits host readiness at {site:?}");
            200
        }
        TerminalExecutionStatus::HostComputationPending(site) => {
            eprintln!("terminal: execution has an unfinished host computation at {site:?}");
            200
        }
    }
}

fn run_cases(arguments: RunArguments, outputs: PathBuf) -> ! {
    let cases = match &arguments.inputs {
        Some(inputs) => {
            let mut cases = match std::fs::read_dir(inputs) {
                Ok(entries) => entries
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == "stdin")
                    })
                    .collect::<Vec<_>>(),
                Err(error) => {
                    eprintln!("cannot read --inputs {}: {error}", inputs.display());
                    std::process::exit(200);
                }
            };
            cases.sort();
            if cases.is_empty() {
                eprintln!("--inputs {} holds no *.stdin files", inputs.display());
                std::process::exit(200);
            }
            cases
        }
        None => arguments.input_files.clone(),
    };
    let outcome = match run_project_cases(CaseRunRequest {
        build_snapshot: arguments
            .build_inputs
            .map(|inputs| compiler::BuildSnapshotRequest::scoped(Vec::new(), inputs)),
        private_entry_ram: arguments.private_entry_ram,
        root_path: arguments.main_path,
        compare_interpreter: arguments.selection == ExecutionSelection::Both,
        psi_work_budget: arguments.psi_work_budget,
        inputs: cases,
        output_directory: outputs.clone(),
        keep_artifacts: arguments.keep,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(compile_failure_status(&error));
        }
    };
    let mut status = 0;
    for case in &outcome.cases {
        let interpreter = match &case.comparison {
            InterpreterComparison::NotRequested => String::new(),
            InterpreterComparison::Declined(reason) => format!("; interp DECLINED ({reason})"),
            InterpreterComparison::Agrees { exit_code } => format!("; interp exit {exit_code}"),
            InterpreterComparison::AgreesCrash { cause } => format!("; interp crash {cause:?}"),
            InterpreterComparison::Disagrees {
                interpreter_exit_code,
            } => {
                status = 201;
                format!("; DIVERGENCE interp exit {interpreter_exit_code}")
            }
            InterpreterComparison::DisagreesCrash { cause } => {
                status = 201;
                format!("; DIVERGENCE interp crash {cause:?}")
            }
            InterpreterComparison::Failed(diagnostics) => {
                status = 201;
                let reasons = diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                format!("; Terminal interpretation FAILED: {}", reasons.join("; "))
            }
        };
        eprintln!(
            "case {}: native {}{interpreter}",
            case.name,
            case.exit.describe()
        );
    }
    eprintln!(
        "{} cases; streams and results.tsv in {}",
        outcome.cases.len(),
        outputs.display()
    );
    if arguments.keep {
        eprintln!("build dir kept: {}", outcome.build_dir.display());
    }
    std::process::exit(status);
}

fn forward_output_or_exit(stdout: &[u8], stderr: &[u8]) {
    let result = (|| -> std::io::Result<()> {
        let mut standard_output = std::io::stdout().lock();
        standard_output.write_all(stdout)?;
        standard_output.flush()?;
        let mut standard_error = std::io::stderr().lock();
        standard_error.write_all(stderr)?;
        standard_error.flush()
    })();
    if let Err(error) = result {
        let _ = writeln!(
            std::io::stderr().lock(),
            "cannot forward program output: {error}"
        );
        std::process::exit(200);
    }
}

fn print_private_entry_ram(private_entry_ram: &[(u32, Vec<u8>)]) {
    for (position, contents) in private_entry_ram {
        eprint!("entry RAM {position}: ");
        for byte in contents {
            eprint!("{byte:02x}");
        }
        eprintln!();
    }
}

fn compile_failure_status(error: &RunError) -> i32 {
    match error {
        RunError::Compilation(diagnostics)
            if matches!(
                CompilerRequestOutcome::of_refusal(diagnostics),
                CompilerRequestOutcome::Incomplete(_)
            ) =>
        {
            202
        }
        _ => 200,
    }
}
