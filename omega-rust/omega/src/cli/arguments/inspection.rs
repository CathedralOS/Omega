use std::path::PathBuf;

pub(crate) struct InspectTerminalArguments {
    pub(crate) machine: String,
    pub(crate) root_path: PathBuf,
    pub(crate) target_name: Option<String>,
}

pub(super) fn parse_inspect_terminal_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<InspectTerminalArguments> {
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    while let Some(argument) = arguments.next() {
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
    Some(InspectTerminalArguments {
        machine: machine?,
        root_path: root_path?,
        target_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn terminal_inspection_rejects_offline_in_every_position() {
        let ordinary = ["--machine", "main", "--target", "linux_x64", "main.omg"];
        assert!(parse_inspect_terminal_arguments(ordinary.iter().map(OsString::from)).is_some());
        for position in 0..=ordinary.len() {
            let mut arguments = ordinary.to_vec();
            arguments.insert(position, "--offline");
            assert!(
                parse_inspect_terminal_arguments(arguments.iter().map(OsString::from)).is_none(),
                "{arguments:?}"
            );
        }
    }
}
