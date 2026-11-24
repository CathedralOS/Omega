//! Files produced by a single-engine `omega run --outputs <new-dir>`.
//! Write the termination record last so a partial stream write cannot certify a run.

use crate::execution::{ExecutionOutcome, ProcessExitObservation, PsiTermination, TerminalOutcome};
use std::{fs, io, path::Path};

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
        ExecutionOutcome::Terminal(outcome) => {
            if let TerminalOutcome::Completed(observation) = outcome {
                write_outputs(
                    directory,
                    &observation.stdout,
                    &observation.stderr,
                    &observation.private_entry_ram,
                )?;
            }
            let termination = PsiTermination::of(outcome);
            let (kind, detail) = termination.columns();
            let record = if matches!(termination, PsiTermination::Returned) {
                format!("psi\t{kind}")
            } else {
                format!("psi\t{kind}\t{detail}")
            };
            (record, termination.complete())
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
