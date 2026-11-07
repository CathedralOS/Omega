//! Text presentation of a checked terminal inspection.

use crate::arguments::InspectTerminalArguments;
use crate::inspection::{
    InspectTerminalRequest, inspect_terminal, inspect_terminal_artifact, terminal_summary,
};

pub(crate) fn run(arguments: InspectTerminalArguments) {
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
            use std::io::Read;
            const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
            let bytes = (|| -> std::io::Result<Vec<u8>> {
                let file = std::fs::File::open(path)?;
                let mut bytes = Vec::new();
                file.take(MAX_ARTIFACT_BYTES + 1).read_to_end(&mut bytes)?;
                if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Terminal inspection artifact exceeds 64 MiB",
                    ));
                }
                Ok(bytes)
            })();
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("cannot read Terminal artifact: {error}");
                    std::process::exit(1);
                }
            };
            (
                "artifact".to_owned(),
                inspect_terminal_artifact(&bytes),
                None,
            )
        }
    };
    match result {
        Ok(inspection) => {
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
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
