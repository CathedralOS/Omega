pub(crate) struct RunArguments {
    pub(crate) both: bool,
    pub(crate) keep: bool,
    pub(crate) target_name: Option<String>,
    /// A directory of `*.stdin` cases run against one compilation.
    pub(crate) inputs: Option<std::path::PathBuf>,
    /// Where each case's streams and `results.tsv` are written.
    pub(crate) outputs: Option<std::path::PathBuf>,
    pub(crate) main_path: std::path::PathBuf,
}

pub(super) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<RunArguments, String> {
    let mut both = false;
    let mut keep = false;
    let mut target_name = None;
    let mut inputs = None;
    let mut outputs = None;
    let mut main_path = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--offline") => return Err("omega run does not support --offline".into()),
            Some("--both") => super::set_flag(&mut both, "--both")?,
            Some("--keep") => super::set_flag(&mut keep, "--keep")?,
            Some(flag @ ("--inputs" | "--outputs")) => {
                let slot = if flag == "--inputs" {
                    &mut inputs
                } else {
                    &mut outputs
                };
                if slot.is_some() {
                    return Err(format!("duplicate {flag}"));
                }
                let directory = arguments
                    .next()
                    .ok_or_else(|| format!("{flag} requires a directory"))?;
                *slot = Some(std::path::PathBuf::from(directory));
            }
            Some("--target") => {
                if target_name.is_some() {
                    return Err("duplicate --target".into());
                }
                let name = arguments.next().ok_or("--target requires a name")?;
                if name == "--offline" {
                    return Err("omega run does not support --offline".into());
                }
                let name = name
                    .into_string()
                    .map_err(|_| "--target requires a UTF-8 target name")?;
                if name.is_empty() || name.starts_with('-') {
                    return Err("--target requires a name, not an option or empty argument".into());
                }
                target_name = Some(name);
            }
            _ => {
                if argument.as_encoded_bytes().starts_with(b"-") {
                    return Err(format!(
                        "unrecognized option `{}`",
                        argument.to_string_lossy()
                    ));
                }
                if main_path.is_some() {
                    return Err(format!(
                        "unexpected extra argument `{}`",
                        argument.to_string_lossy()
                    ));
                }
                main_path = Some(std::path::PathBuf::from(argument));
            }
        }
    }
    let main_path = main_path.ok_or_else(|| "missing root Omega source path".to_owned())?;
    if inputs.is_some() != outputs.is_some() {
        return Err("--inputs and --outputs are used together".into());
    }
    if inputs.is_some() && target_name.is_some() {
        return Err("--inputs runs on the host; it cannot take --target".into());
    }
    Ok(RunArguments {
        both,
        keep,
        target_name,
        inputs,
        outputs,
        main_path,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;

    fn parse(arguments: &[&str]) -> Result<super::RunArguments, String> {
        parse_arguments(arguments.iter().map(std::ffi::OsString::from))
    }

    #[test]
    fn inputs_and_outputs_select_a_case_run() {
        let arguments = parse(&["--both", "--inputs", "data", "--outputs", "out", "main.omg"])
            .expect("case run parses");
        assert!(arguments.both);
        assert_eq!(
            arguments.inputs.as_deref(),
            Some(std::path::Path::new("data"))
        );
        assert_eq!(
            arguments.outputs.as_deref(),
            Some(std::path::Path::new("out"))
        );
    }

    #[test]
    fn inputs_need_outputs_and_exclude_a_foreign_target() {
        assert!(parse(&["--inputs", "data", "main.omg"]).is_err());
        assert!(parse(&["--outputs", "out", "main.omg"]).is_err());
        assert!(
            parse(&[
                "--inputs",
                "d",
                "--outputs",
                "o",
                "--target",
                "linux_x86_64",
                "main.omg"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--inputs",
                "d",
                "--inputs",
                "e",
                "--outputs",
                "o",
                "main.omg"
            ])
            .is_err()
        );
    }
}
