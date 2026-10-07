pub(crate) struct RunArguments {
    pub(crate) both: bool,
    pub(crate) keep: bool,
    pub(crate) target_name: Option<String>,
    pub(crate) main_path: std::path::PathBuf,
}

pub(super) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<RunArguments, String> {
    let mut both = false;
    let mut keep = false;
    let mut target_name = None;
    let mut main_path = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--offline") => return Err("omega run does not support --offline".into()),
            Some("--both") => super::set_flag(&mut both, "--both")?,
            Some("--keep") => super::set_flag(&mut keep, "--keep")?,
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
    Ok(RunArguments {
        both,
        keep,
        target_name,
        main_path,
    })
}
