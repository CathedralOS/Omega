//! Parse the engine, inputs and observation destination for an application run.

use crate::execution::ExecutionSelection;

pub(crate) struct RunArguments {
    pub(crate) selection: ExecutionSelection,
    pub(crate) build_directories: platform_custody::directory_mappings::DirectoryMappings,
    pub(crate) run_directories: platform_custody::directory_mappings::DirectoryMappings,
    pub(crate) build_inputs: Option<package_compilation::BuildSourceCaptureRequest>,
    pub(crate) psi_work_budget: Option<std::num::NonZeroU64>,
    pub(crate) private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub(crate) keep: bool,
    pub(crate) target_name: Option<String>,
    pub(crate) inputs: Option<std::path::PathBuf>, // Directory of *.stdin cases run against one compilation.
    pub(crate) input_files: Vec<std::path::PathBuf>, // Explicit case files (--input, repeatable), named by file stem.
    pub(crate) outputs: Option<std::path::PathBuf>, // Where each case's streams and results.tsv are written.
    pub(crate) trusted_artifact: Option<String>,
    pub(crate) main_path: std::path::PathBuf,
}

macro_rules! run_help {
    (synopsis) => {
        "omega run [--engine <native|psi>] [--build-directories-readonly <host::/guest>]... [--build-directories-readwrite <host::/guest>]... [--run-directories-readonly <host::/guest>]... [--run-directories-readwrite <host::/guest>]... [--psi-work-budget <positive-u64>] [--keep] [--build-input <path>]... [--optional-build-input <path>]... [--entry-ram <position>=<hex-bytes>]... [--target <name>] [--inputs <dir> | --input <file>...] [--outputs <dir>] [--trusted-artifact <manifest-id>] <root.omg|program.psi>"
    };
    (host_packages) => {
        "For saved Psi, OMEGA_HOST_PACKAGE_LOCK selects an absolute path to the host's reviewed omega.lock. Set OMEGA_HOST_PACKAGE_LOCK_SHA256 to its approved SHA-256 digest."
    };
    (work_budget) => {
        "--psi-work-budget applies to --engine psi."
    };
}
pub(super) use run_help;

pub(super) const USAGE: &str = concat!(
    "usage: ",
    run_help!(synopsis),
    "\n",
    run_help!(work_budget),
    "\n",
    run_help!(host_packages),
    "\nFor a single engine, --outputs <new-dir> writes raw stdout, stderr and termination.tsv. The command succeeds when it records a completed execution, including a guest crash or nonzero exit.\nWith --target, the native engine emits artifacts\nwithout running the program, even when the target matches this host.\nWith --engine psi, --target selects the target environment for Psi execution.",
);

pub(super) fn parse_arguments(
    arguments: &mut super::Arguments,
) -> Result<Option<RunArguments>, String> {
    let mut build_inputs = Vec::new();
    let mut build_directories = Default::default();
    let mut run_directories = Default::default();
    let mut engine = None;
    let mut psi_work_budget = None;
    let mut private_entry_ram = Vec::new();
    let mut help = false;
    let mut keep = false;
    let mut target_name = None;
    let mut inputs = None;
    let mut input_files = Vec::new();
    let mut outputs = None;
    let mut main_path = None;
    let mut trusted_artifact = None;
    while let Some(argument) = arguments.next_argument() {
        match argument.to_str() {
            Some(flag @ ("--build-directories-readonly" | "--build-directories-readwrite")) => {
                super::parse_directory(arguments, flag, &mut build_directories)?;
            }
            Some(flag @ ("--run-directories-readonly" | "--run-directories-readwrite")) => {
                super::parse_directory(arguments, flag, &mut run_directories)?;
            }
            Some(flag @ ("--build-input" | "--optional-build-input")) => {
                build_inputs.push(super::parse_build_input(arguments, flag)?);
            }
            Some("--help" | "-h") => super::set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--offline") => return Err("omega run does not support --offline".into()),
            Some("--engine") => super::set_once(&mut engine, "--engine", || {
                let value =
                    super::option_value(arguments).ok_or("--engine requires `native` or `psi`")?;
                match value.to_str() {
                    Some("native") => Ok(ExecutionSelection::Native),
                    Some("psi") => Ok(ExecutionSelection::Terminal),
                    _ => Err(format!(
                        "--engine requires `native` or `psi`, not `{}`",
                        value.to_string_lossy()
                    )),
                }
            })?,
            Some("--keep") => super::set_flag(&mut keep, "--keep")?,
            Some("--psi-work-budget") => {
                super::set_once(&mut psi_work_budget, "--psi-work-budget", || {
                    let value = super::option_value(arguments)
                        .ok_or("--psi-work-budget requires a positive u64")?
                        .into_string()
                        .map_err(|_| "--psi-work-budget requires a positive u64")?;
                    value
                        .parse::<std::num::NonZeroU64>()
                        .map_err(|_| "--psi-work-budget requires a positive u64".into())
                })?
            }
            Some("--entry-ram") => {
                let value = arguments
                    .next_value()
                    .ok_or("--entry-ram requires POSITION=HEX_BYTES")?
                    .into_string()
                    .map_err(|_| "--entry-ram requires UTF-8 POSITION=HEX_BYTES")?;
                let (position, encoded) = value
                    .split_once('=')
                    .ok_or("--entry-ram requires POSITION=HEX_BYTES")?;
                let position_text = position;
                let position = position.parse::<u32>().map_err(
                    |_| "--entry-ram position must be an unsigned source parameter position",
                )?;
                if position.to_string() != position_text {
                    return Err("--entry-ram position must be canonical unsigned decimal".into());
                }
                if encoded.is_empty() || encoded.len() % 2 != 0 || encoded.len() > 131072 {
                    return Err(
                        "--entry-ram requires nonempty complete bytes, bounded to 64 KiB".into(),
                    );
                }
                let contents = encoded
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| {
                        let digit = |byte: u8| match byte {
                            b'0'..=b'9' => Some(byte - b'0'),
                            b'a'..=b'f' => Some(byte - b'a' + 10),
                            _ => None,
                        };
                        Ok(
                            digit(pair[0]).ok_or("--entry-ram hex must be canonical lowercase")?
                                * 16
                                + digit(pair[1])
                                    .ok_or("--entry-ram hex must be canonical lowercase")?,
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                private_entry_ram
                    .push(psi_entry_memory::PrivateEntryRamInitialization { position, contents });
            }
            Some("--trusted-artifact") => {
                super::parse_trusted_artifact(arguments, &mut trusted_artifact)?
            }
            Some("--input") => {
                let file = super::option_value(arguments).ok_or("--input requires a file")?;
                input_files.push(std::path::PathBuf::from(file));
            }
            Some(flag @ ("--inputs" | "--outputs")) => {
                let slot = if flag == "--inputs" {
                    &mut inputs
                } else {
                    &mut outputs
                };
                super::set_once(slot, flag, || {
                    super::option_value(arguments)
                        .map(std::path::PathBuf::from)
                        .ok_or_else(|| format!("{flag} requires a directory"))
                })?;
            }
            Some("--target") => super::set_once(&mut target_name, "--target", || {
                let name = arguments.next_value().ok_or("--target requires a name")?;
                if name == "--offline" {
                    return Err("omega run does not support --offline".into());
                }
                let name = name
                    .into_string()
                    .map_err(|_| "--target requires a UTF-8 target name")?;
                if name.is_empty() || name.starts_with('-') {
                    return Err("--target requires a name, not an option or empty argument".into());
                }
                Ok(name)
            })?,
            _ => super::positional(argument, &mut main_path, |path| {
                Ok(std::path::PathBuf::from(path))
            })?,
        }
    }
    let selection = engine.unwrap_or_default();
    let terminal = selection == ExecutionSelection::Terminal;
    if !terminal && !run_directories.is_empty() {
        return Err("Product directory mappings require --engine psi".into());
    }
    let cases = inputs.is_some() || !input_files.is_empty();
    if inputs.is_some() && !input_files.is_empty() {
        return Err("use either --inputs <dir> or --input <file>, not both".into());
    }
    if cases && outputs.is_none() {
        return Err("--inputs or --input is used together with --outputs".into());
    }
    if outputs.is_some() && !cases && !terminal && target_name.is_some() {
        return Err("--outputs requires an executing single engine or an input case list".into());
    }
    if cases && target_name.is_some() && !terminal {
        return Err(
            "native case runs require host execution; --target with cases requires --engine psi"
                .into(),
        );
    }
    if psi_work_budget.is_some() && !terminal {
        return Err("--psi-work-budget requires --engine psi".into());
    }
    if help {
        return Ok(None);
    }
    let main_path = main_path.ok_or_else(|| "missing root Omega source path".to_owned())?;
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(package_compilation::BuildSourceCaptureRequest::new(
            build_inputs,
        )?)
    };
    if !build_directories.is_empty()
        && main_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
    {
        return Err("Build directory mappings require a source Build".into());
    }
    if trusted_artifact.is_some()
        && !main_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
    {
        return Err("--trusted-artifact applies to a saved .psi program".into());
    }
    Ok(Some(RunArguments {
        trusted_artifact,
        build_inputs,
        build_directories,
        run_directories,
        private_entry_ram,
        selection,
        psi_work_budget,
        keep,
        target_name,
        inputs,
        input_files,
        outputs,
        main_path,
    }))
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;
    use crate::arguments::Arguments;
    use crate::execution::ExecutionSelection;

    fn parse(arguments: &[&str]) -> Result<super::RunArguments, String> {
        parse_arguments(&mut Arguments::new(
            arguments.iter().map(std::ffi::OsString::from),
        ))?
        .ok_or_else(|| "unexpected --help".to_owned())
    }

    #[test]
    fn every_execution_route_retains_the_same_caller_source_inventory() {
        use package_compilation::BuildSourceCaptureObligation::{Optional, Required};
        for route in [
            vec!["--engine", "native"],
            vec!["--engine", "psi"],
            vec![
                "--engine",
                "native",
                "--input",
                "case.stdin",
                "--outputs",
                "out",
            ],
        ] {
            let mut arguments = route;
            arguments.extend([
                "--build-input",
                "main.omg",
                "--build-input",
                "build.omg",
                "--optional-build-input",
                "settings.txt",
                "main.omg",
            ]);
            let parsed = parse(&arguments).unwrap();
            let captured = parsed.build_inputs.unwrap();
            assert_eq!(
                captured.entries().collect::<Vec<_>>(),
                [
                    (b"build.omg".as_slice(), Required),
                    (b"main.omg".as_slice(), Required),
                    (b"settings.txt".as_slice(), Optional),
                ]
            );
        }
    }

    #[test]
    fn run_source_inventory_refuses_missing_duplicate_or_noncanonical_entries() {
        for arguments in [
            vec!["main.omg", "--build-input"],
            vec!["--build-input", "../private.omg", "main.omg"],
            vec![
                "--build-input",
                "main.omg",
                "--build-input",
                "main.omg",
                "main.omg",
            ],
            vec![
                "--build-input",
                "src",
                "--build-input",
                "src/main.omg",
                "main.omg",
            ],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn inputs_and_outputs_select_a_case_run() {
        let arguments = parse(&[
            "--engine",
            "native",
            "--inputs",
            "data",
            "--outputs",
            "out",
            "main.omg",
        ])
        .expect("case run parses");
        assert_eq!(arguments.selection, ExecutionSelection::Native);
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
    fn inputs_need_outputs_and_only_psi_can_select_a_target() {
        let psi = parse(&[
            "--engine",
            "psi",
            "--target",
            "linux_arm64",
            "--inputs",
            "data",
            "--outputs",
            "out",
            "main.omg",
        ])
        .unwrap();
        assert_eq!(psi.target_name.as_deref(), Some("linux_arm64"));
        assert!(parse(&["--inputs", "data", "main.omg"]).is_err());
        for engine in ["native", "psi"] {
            assert!(parse(&["--engine", engine, "--outputs", "out", "main.omg"]).is_ok());
        }
        assert!(parse(&["--target", "linux_x86_64", "--outputs", "out", "main.omg"]).is_err());
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
        assert!(
            parse(&[
                "--inputs",
                "d",
                "--input",
                "a.txt",
                "--outputs",
                "o",
                "main.omg"
            ])
            .is_err()
        );
    }

    #[test]
    fn repeated_input_files_select_a_case_run() {
        let arguments = parse(&[
            "--input",
            "a.txt",
            "--input",
            "b.txt",
            "--outputs",
            "o",
            "main.omg",
        ])
        .expect("explicit case files parse");
        assert_eq!(arguments.input_files.len(), 2);
        assert!(arguments.inputs.is_none());
    }

    #[test]
    fn psi_work_budget_is_explicit_and_order_independent() {
        let arguments = parse(&[
            "--psi-work-budget",
            "100000000",
            "--engine",
            "psi",
            "main.omg",
        ])
        .expect("positive host reference allowance parses");
        assert_eq!(
            arguments.psi_work_budget.map(std::num::NonZeroU64::get),
            Some(100_000_000)
        );
        assert!(
            parse(&["--engine", "psi", "main.omg"])
                .expect("default allowance parses")
                .psi_work_budget
                .is_none()
        );
    }

    #[test]
    fn engine_selects_one_execution_and_defaults_to_native() {
        for (arguments, expected) in [
            (vec!["main.omg"], ExecutionSelection::Native),
            (
                vec!["--engine", "native", "main.omg"],
                ExecutionSelection::Native,
            ),
            (
                vec!["--engine", "psi", "main.omg"],
                ExecutionSelection::Terminal,
            ),
            (
                vec!["main.omg", "--engine", "psi"],
                ExecutionSelection::Terminal,
            ),
            (
                vec!["--engine", "psi", "--target", "linux_x86_64", "main.omg"],
                ExecutionSelection::Terminal,
            ),
            (
                vec!["--engine", "psi", "--psi-work-budget", "5", "main.omg"],
                ExecutionSelection::Terminal,
            ),
        ] {
            assert_eq!(
                parse(&arguments).expect("engine parses").selection,
                expected
            );
        }
    }

    #[test]
    fn engine_rejects_both_unknown_names_and_duplicates() {
        for arguments in [
            vec!["--both", "main.omg"],
            vec!["--engine", "jit", "main.omg"],
            vec!["--engine", "terminal", "main.omg"],
            vec!["--engine", "main.omg"],
            vec!["--engine", "native", "--engine", "psi", "main.omg"],
            vec!["--engine", "native", "--psi-work-budget", "5", "main.omg"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?} should reject");
        }
    }

    #[test]
    fn psi_work_budget_rejects_invalid_or_unused_requests() {
        assert!(
            parse(&[
                "--engine",
                "psi",
                "--terminal-fuel-allowance",
                "1",
                "main.omg"
            ])
            .is_err()
        );
        for value in ["0", "-1", "many", "18446744073709551616"] {
            assert!(parse(&["--engine", "psi", "--psi-work-budget", value, "main.omg"]).is_err());
        }
        for arguments in [
            vec!["--psi-work-budget", "--engine", "psi", "main.omg"],
            vec!["--psi-work-budget", "1", "main.omg"],
            vec![
                "--engine",
                "native",
                "--target",
                "linux_x86_64",
                "--psi-work-budget",
                "1",
                "main.omg",
            ],
            vec![
                "--engine",
                "psi",
                "--psi-work-budget",
                "1",
                "--psi-work-budget",
                "2",
                "main.omg",
            ],
        ] {
            assert!(parse(&arguments).is_err());
        }
    }
}
