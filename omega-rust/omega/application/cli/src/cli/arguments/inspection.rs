use std::path::PathBuf;

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

pub(super) fn parse_inspect_terminal_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<InspectTerminalArguments> {
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut artifact_path = None;
    let mut output = None;
    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            if output.is_some() {
                return None;
            }
            output = super::option_value(&mut arguments).map(PathBuf::from);
            output.as_ref()?;
            continue;
        }
        if argument == "--artifact" {
            if artifact_path.is_some() {
                return None;
            }
            artifact_path = super::option_value(&mut arguments).map(PathBuf::from);
            artifact_path.as_ref()?;
            continue;
        }
        if argument == "--machine" {
            if machine.is_some() {
                return None;
            }
            machine =
                super::option_value(&mut arguments).and_then(|value| value.into_string().ok());
            machine.as_ref()?;
            continue;
        }
        if argument == "--target" {
            if target_name.is_some() {
                return None;
            }
            target_name =
                super::option_value(&mut arguments).and_then(|value| value.into_string().ok());
            target_name.as_ref()?;
            continue;
        }
        if root_path.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        root_path = Some(PathBuf::from(argument));
    }
    match artifact_path {
        Some(path)
            if root_path.is_none()
                && machine.is_none()
                && target_name.is_none()
                && output.is_none() =>
        {
            Some(InspectTerminalArguments::Artifact { path })
        }
        Some(_) => None,
        None => Some(InspectTerminalArguments::Source {
            machine: machine?,
            root_path: root_path?,
            target_name,
            output,
        }),
    }
}
