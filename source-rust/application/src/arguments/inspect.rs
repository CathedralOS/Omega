// Parse arguments for `omega psi inspect`, selecting either source compilation (`--machine`) or a saved `.psi` file (`--artifact`).

use std::path::PathBuf;

pub(super) const USAGE: &str = "usage: omega psi inspect --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega psi inspect --artifact <file> [--max-psi-input-bytes <bytes>]";

pub(crate) enum InspectTerminalArguments {
    Source {
        machine: String,
        root_path: PathBuf,
        target_name: Option<String>,
        output: Option<PathBuf>,
    },
    Artifact {
        path: PathBuf,
        max_psi_input_bytes: Option<u64>,
    },
}

pub(super) fn parse_arguments(
    arguments: &mut super::Arguments,
) -> Result<Option<InspectTerminalArguments>, String> {
    let mut help = false;
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut artifact_path = None;
    let mut max_psi_input_bytes = None;
    let mut output = None;
    while let Some(argument) = arguments.next_argument() {
        match argument.to_str() {
            Some("--max-psi-input-bytes") => super::parse_byte_limit(arguments, "--max-psi-input-bytes", &mut max_psi_input_bytes)?,
            Some("--help" | "-h") => super::set_flag(&mut help, &argument.to_string_lossy())?,
            Some(flag @ ("--output" | "--artifact")) => {
                let slot = if flag == "--output" {
                    &mut output
                } else {
                    &mut artifact_path
                };
                super::set_once(slot, flag, || {
                    super::take_value(arguments, flag).map(PathBuf::from)
                })?;
            }
            Some(flag @ ("--machine" | "--target")) => {
                let slot = if flag == "--machine" {
                    &mut machine
                } else {
                    &mut target_name
                };
                super::set_once(slot, flag, || super::take_text(arguments, flag))?;
            }
            _ => super::positional(argument, &mut root_path, |path| Ok(PathBuf::from(path)))?,
        }
    }
    if help {
        return Ok(None);
    }
    match artifact_path {
        Some(path)
            if root_path.is_none()
                && machine.is_none()
                && target_name.is_none()
                && output.is_none() =>
        {
            Ok(Some(InspectTerminalArguments::Artifact { path, max_psi_input_bytes }))
        }
        Some(_) => Err("--artifact cannot be combined with source inspection arguments".into()),
        None if max_psi_input_bytes.is_some() => Err("--max-psi-input-bytes requires --artifact for Psi inspection".into()),
        None => Ok(Some(InspectTerminalArguments::Source {
            machine: machine.ok_or_else(|| "missing --machine <qualified>".to_owned())?,
            root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
            target_name,
            output,
        })),
    }
}
