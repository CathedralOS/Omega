//! Files produced by a single-engine `omega run --outputs <new-dir>`.
//! Write the termination record last so a partial stream write cannot certify a run.

use crate::execution::{ExecutionOutcome, ProcessExitObservation, TerminalOutcome};
use std::{fs, io, path::Path};
use terminal_runtime::{TerminalExecutionResult, TerminalExecutionStatus};

pub(super) fn write(directory: &Path, execution: &ExecutionOutcome) -> io::Result<bool> {
    fs::create_dir(directory)?;
    let (termination, complete) = match execution {
        ExecutionOutcome::TargetOnly { .. } => {
            return Err(io::Error::other("the selected target did not run"));
        }
        ExecutionOutcome::Host {
            output,
            exit,
            can_end_the_process,
            private_entry_ram,
            ..
        } => {
            write_outputs(directory, &output.stdout, &output.stderr, private_entry_ram)?;
            let trap = match exit.matches_crash_presentation(terminal_psi::CrashCause::Trap) {
                Some(true) => "trap",
                Some(false) => "other",
                None => "unknown",
            };
            let status = match exit {
                ProcessExitObservation::Exited(code) => format!("exit\t{code}"),
                ProcessExitObservation::Signaled(signal) => format!("signal\t{signal}"),
                ProcessExitObservation::Terminated => "terminated".into(),
            };
            let process_exit = match can_end_the_process {
                Some(true) => "process-exit-possible",
                Some(false) => "process-exit-excluded",
                None => "process-exit-unknown",
            };
            (format!("native\t{status}\t{trap}\t{process_exit}"), true)
        }
        ExecutionOutcome::Terminal(TerminalOutcome::Completed(observation)) => {
            write_outputs(
                directory,
                &observation.stdout,
                &observation.stderr,
                &observation.private_entry_ram,
            )?;
            match &observation.status {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                    ("psi\treturn".into(), true)
                }
                TerminalExecutionStatus::ExternalCompletion(completion) => {
                    (format!("psi\texit\t{}", completion.return_code), true)
                }
                TerminalExecutionStatus::Crashed(crash) => {
                    (format!("psi\tcrash\t{:?}", crash.cause), true)
                }
                TerminalExecutionStatus::Complete(result) => {
                    (format!("psi\tincomplete\tentry returned {result:?}"), false)
                }
                TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                    (format!("psi\tincomplete\t{exhaustion}"), false)
                }
                TerminalExecutionStatus::AwaitingEffect(site) => (
                    format!("psi\tincomplete\tawaiting host readiness at {site:?}"),
                    false,
                ),
                TerminalExecutionStatus::HostComputationPending(site) => (
                    format!("psi\tincomplete\tunfinished host computation at {site:?}"),
                    false,
                ),
            }
        }
        ExecutionOutcome::Terminal(TerminalOutcome::Unavailable(reason)) => {
            (format!("psi\tunavailable\t{reason}"), false)
        }
        ExecutionOutcome::Terminal(TerminalOutcome::Failed(reason)) => {
            (format!("psi\tfailed\t{reason}"), false)
        }
    };
    fs::write(
        directory.join("termination.tsv"),
        format!("{termination}\n"),
    )?;
    Ok(complete)
}

fn write_outputs(
    directory: &Path,
    stdout: &[u8],
    stderr: &[u8],
    memory: &[(u32, Vec<u8>)],
) -> io::Result<()> {
    fs::write(directory.join("stdout"), stdout)?;
    fs::write(directory.join("stderr"), stderr)?;
    for (position, contents) in memory {
        fs::write(
            directory.join(format!("entry-ram.{position}.bin")),
            contents,
        )?;
    }
    Ok(())
}
