// Parse arguments for `omega psi inspect`, selecting either source compilation (`--machine`) or a saved `.psi` file (`--artifact`).

use std::path::PathBuf;

pub(super) const USAGE: &str = "usage: omega psi inspect --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega psi inspect --artifact <file>";

pub(crate) enum InspectTerminalArguments {
    Source {
        machine: String,
        root_path: PathBuf,
        target_name: Option<String>,
        output: Option<PathBuf>,
    },
    Artifact {
        path: PathBuf,
    },
}

pub(super) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Option<InspectTerminalArguments>, String> {
    let mut help = false;
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut artifact_path = None;
    let mut output = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help" | "-h") => super::set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--output") => {
                if output.is_some() {
                    return Err("duplicate --output".into());
                }
                output = Some(PathBuf::from(super::take_value(
                    &mut arguments,
                    "--output",
                )?));
            }
            Some("--artifact") => {
                if artifact_path.is_some() {
                    return Err("duplicate --artifact".into());
                }
                artifact_path = Some(PathBuf::from(super::take_value(
                    &mut arguments,
                    "--artifact",
                )?));
            }
            Some("--machine") => {
                if machine.is_some() {
                    return Err("duplicate --machine".into());
                }
                machine = Some(super::take_text(&mut arguments, "--machine")?);
            }
            Some("--target") => {
                if target_name.is_some() {
                    return Err("duplicate --target".into());
                }
                target_name = Some(super::take_text(&mut arguments, "--target")?);
            }
            _ => {
                if argument.as_encoded_bytes().starts_with(b"-") {
                    return Err(format!(
                        "unrecognized option `{}`",
                        argument.to_string_lossy()
                    ));
                }
                if root_path.is_some() {
                    return Err(format!(
                        "unexpected extra argument `{}`",
                        argument.to_string_lossy()
                    ));
                }
                root_path = Some(PathBuf::from(argument));
            }
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
            Ok(Some(InspectTerminalArguments::Artifact { path }))
        }
        Some(_) => Err("--artifact cannot be combined with source inspection arguments".into()),
        None => Ok(Some(InspectTerminalArguments::Source {
            machine: machine.ok_or_else(|| "missing --machine <qualified>".to_owned())?,
            root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
            target_name,
            output,
        })),
    }
}
