/*

Console adapter for `omega psi inspect`.

We either compile a source program and extract the requested machine's Terminal
Psi module (`InspectTerminalArguments::Source`, optionally writing the encoded
`PSITERM\0` artifact to `--output`), or decode an existing `.psi` file from
disk (`InspectTerminalArguments::Artifact`). In both cases we print the
human-readable `terminal_summary` followed by the lowercase hex
`program_fingerprint`.

The receiving host can limit the file read before decoding.

*/

use std::path::Path;

use crate::arguments::InspectTerminalArguments;
use crate::inspection::{
    InspectTerminalRequest, inspect_terminal, inspect_terminal_artifact, terminal_summary,
};

use crate::diagnostic_output::DiagnosticOutput;

pub(crate) fn run(arguments: InspectTerminalArguments, diagnostics: DiagnosticOutput) {
    let (selected, result, output) = match arguments {
        InspectTerminalArguments::Source {
            machine,
            root_path,
            target_name,
            output,
        } => {
            let request = InspectTerminalRequest {
                root_path,
                machine,
                target_name,
            };
            (request.machine.clone(), inspect_terminal(&request), output)
        }
        InspectTerminalArguments::Artifact { path, max_psi_input_bytes } => {
            let bytes = read_artifact_or_exit(&path, max_psi_input_bytes);
            (
                "artifact".to_owned(),
                inspect_terminal_artifact(&bytes),
                None,
            )
        }
    };
    let inspection = match result {
        Ok(inspection) => inspection,
        Err(error) => {
            use crate::inspection::InspectTerminalError;
            match &error {
                InspectTerminalError::Diagnostics(rows) => eprintln!("{}", diagnostics.diagnostics(rows)),
                InspectTerminalError::Artifact(_) => eprintln!("{error}"),
            }
            std::process::exit(1);
        }
    };
    if let Some(path) = output
        && let Err(error) = std::fs::write(path, inspection.artifact.to_bytes())
    {
        eprintln!("cannot write Psi artifact: {error}");
        std::process::exit(1);
    }
    print!(
        "{}",
        terminal_summary(&selected, inspection.artifact.semantic_module())
    );
    print!("psi program_fingerprint=");
    for byte in inspection
        .artifact
        .view()
        .identity()
        .program_fingerprint
        .as_bytes()
    {
        print!("{byte:02x}");
    }
    println!();
}

fn read_artifact_or_exit(path: &Path, max_bytes: Option<u64>) -> Vec<u8> {
    crate::saved_program::input::read_artifact(path, max_bytes).unwrap_or_else(|error| {
        eprintln!("cannot read Psi artifact: {error}");
        std::process::exit(1);
    })
}
