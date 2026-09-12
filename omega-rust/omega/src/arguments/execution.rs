pub(crate) struct RunArguments {
    pub(crate) both: bool,
    pub(crate) keep: bool,
    pub(crate) target_name: Option<String>,
    pub(crate) main_path: std::path::PathBuf,
}

pub(super) fn parse_arguments(
    arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<RunArguments, String> {
    let mut arguments = arguments
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    // This command has no offline option. Reject it before extracting values
    // or positionals so it cannot disappear among ignored arguments.
    if arguments.iter().any(|argument| argument == "--offline") {
        return Err("omega run does not support --offline".to_owned());
    }
    let both = arguments.iter().any(|argument| argument == "--both");
    let keep = arguments.iter().any(|argument| argument == "--keep");
    arguments.retain(|argument| argument != "--both" && argument != "--keep");
    let target_name =
        if let Some(index) = arguments.iter().position(|argument| argument == "--target") {
            let name = arguments
                .get(index + 1)
                .cloned()
                .ok_or_else(|| "--target requires a name".to_owned())?;
            arguments.drain(index..=index + 1);
            Some(name)
        } else {
            None
        };
    let main_path = arguments
        .first()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "missing root Omega source path".to_owned())?;
    Ok(RunArguments {
        both,
        keep,
        target_name,
        main_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_rejects_offline_including_after_the_source_or_as_a_value() {
        for arguments in [
            vec!["--offline", "main.omg"],
            vec!["main.omg", "--offline"],
            vec!["--both", "main.omg", "--offline"],
            vec!["--target", "--offline", "main.omg"],
        ] {
            let result = parse_arguments(arguments.iter().map(std::ffi::OsString::from));
            assert!(matches!(result, Err(error) if error.contains("does not support --offline")));
        }
    }

    #[test]
    fn probe_preserves_ordinary_options() {
        let arguments = parse_arguments(
            ["--both", "main.omg", "--keep", "--target", "linux_x64"]
                .into_iter()
                .map(std::ffi::OsString::from),
        )
        .unwrap();
        assert!(arguments.both);
        assert!(arguments.keep);
        assert_eq!(arguments.main_path, std::path::PathBuf::from("main.omg"));
        assert_eq!(arguments.target_name.as_deref(), Some("linux_x64"));
    }
}
