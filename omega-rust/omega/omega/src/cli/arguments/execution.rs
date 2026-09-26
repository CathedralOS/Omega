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
            Some("--both") => {
                if both {
                    return Err("duplicate --both".into());
                }
                both = true;
            }
            Some("--keep") => {
                if keep {
                    return Err("duplicate --keep".into());
                }
                keep = true;
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
    Ok(RunArguments {
        both,
        keep,
        target_name,
        main_path,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;

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
            ["--both", "main.omg", "--keep", "--target", "linux_x86_64"]
                .into_iter()
                .map(std::ffi::OsString::from),
        )
        .unwrap();
        assert!(arguments.both);
        assert!(arguments.keep);
        assert_eq!(arguments.main_path, std::path::PathBuf::from("main.omg"));
        assert_eq!(arguments.target_name.as_deref(), Some("linux_x86_64"));
    }

    #[test]
    fn run_rejects_extra_roots_unknown_options_and_duplicate_options() {
        for arguments in [
            vec!["main.omg", "other.omg"],
            vec!["main.omg", "--bogus"],
            vec!["--bogus", "main.omg"],
            vec!["main.omg", "--both", "--both"],
            vec!["--keep", "main.omg", "--keep"],
            vec![
                "--target",
                "linux_x86_64",
                "main.omg",
                "--target",
                "windows_x86_64",
            ],
        ] {
            assert!(
                parse_arguments(arguments.iter().map(std::ffi::OsString::from)).is_err(),
                "unexpectedly accepted {arguments:?}"
            );
        }
    }

    #[test]
    fn run_requires_a_root_and_a_nonempty_target_name_not_an_option() {
        for arguments in [
            vec![],
            vec!["--both"],
            vec!["main.omg", "--target"],
            vec!["main.omg", "--target", ""],
            vec!["main.omg", "--target", "--keep"],
        ] {
            assert!(
                parse_arguments(arguments.iter().map(std::ffi::OsString::from)).is_err(),
                "unexpectedly accepted {arguments:?}"
            );
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn run_preserves_non_utf8_paths_but_rejects_non_utf8_target_names() {
        use std::ffi::OsString;

        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(b"source-\xff.omg".to_vec())
        };
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[
                b's' as u16,
                0xd800,
                b'.' as u16,
                b'o' as u16,
                b'm' as u16,
                b'g' as u16,
            ])
        };
        let parsed = parse_arguments([path.clone()].into_iter()).unwrap();
        assert_eq!(parsed.main_path.as_os_str(), path);
        assert!(parse_arguments(["--target".into(), path, "main.omg".into()].into_iter()).is_err());
    }
}
