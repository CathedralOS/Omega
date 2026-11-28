//! Files produced by a single `omega run --outputs <new-dir>`.
//! Write the termination record last so a partial stream write cannot certify a run.

use crate::execution::{PsiTermination, TerminalOutcome};
use std::{fs, io, path::Path};

pub(super) fn write(directory: &Path, outcome: &TerminalOutcome) -> io::Result<bool> {
    fs::create_dir(directory)?;
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
    fs::write(directory.join("termination.tsv"), format!("{record}\n"))?;
    Ok(termination.complete())
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
