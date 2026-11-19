/*

Console adapter for `omega psi inspect`.

We either compile a source program and extract the requested machine's Terminal
Psi module (`InspectTerminalArguments::Source`, optionally writing the encoded
`PSITERM\0` artifact to `--output`), or decode an existing `.psi` file from
disk (`InspectTerminalArguments::Artifact`). In both cases we print the
human-readable `terminal_summary` followed by the lowercase hex
`program_fingerprint`.

When reading an artifact from disk, we cap the read at `MAX_ARTIFACT_BYTES`
(64 MiB) before allocation grows unboundedly, so passing an unrelated large
file fails fast with an explicit size error.

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
        InspectTerminalArguments::Artifact { path } => {
            let bytes = read_artifact_or_exit(&path);
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
                InspectTerminalError::Build(build) => eprintln!("{}", diagnostics.build_error(build)),
                _ => eprintln!("{error}"),
            }
            std::process::exit(1);
        }
    };
    if let Some(path) = output
        && let Err(error) = std::fs::write(path, inspection.artifact.to_bytes())
    {
        eprintln!("cannot write Terminal artifact: {error}");
        std::process::exit(1);
    }
    print!(
        "{}",
        terminal_summary(&selected, inspection.artifact.semantic_module())
    );
    print!("terminal program_fingerprint=");
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

fn read_artifact_or_exit(path: &Path) -> Vec<u8> {
    crate::saved_program::input::read_artifact(path).unwrap_or_else(|error| {
        eprintln!("cannot read Psi artifact: {error}");
        std::process::exit(1);
    })
}
