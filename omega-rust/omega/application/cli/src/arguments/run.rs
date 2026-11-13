// Parse arguments for `omega run`.
//
// `--both` is a differential comparison request (native vs. Terminal Psi), not
// a third engine name, so combining `--engine` with `--both` is rejected up
// front. Case runs (`--inputs` or `--input` paired with `--outputs`) always
// execute on the host and therefore reject `--target`.

use crate::execution::ExecutionSelection;

pub(crate) struct RunArguments {
    pub(crate) selection: ExecutionSelection, // Native unless --engine terminal or --both was named.
    pub(crate) build_inputs: Option<package_compilation::BuildSourceCaptureRequest>,
    pub(crate) psi_work_budget: Option<std::num::NonZeroU64>,
    pub(crate) private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    pub(crate) keep: bool,
    pub(crate) target_name: Option<String>,
    pub(crate) inputs: Option<std::path::PathBuf>, // Directory of *.stdin cases run against one compilation.
    pub(crate) input_files: Vec<std::path::PathBuf>, // Explicit case files (--input, repeatable), named by file stem.
    pub(crate) outputs: Option<std::path::PathBuf>, // Where each case's streams and results.tsv are written.
    pub(crate) main_path: std::path::PathBuf,
}

pub(super) const USAGE: &str = "usage: omega run [--engine <native|terminal> | --both] [--psi-work-budget <positive-u64>] [--keep] [--build-input <path>]... [--optional-build-input <path>]... [--entry-ram <position>=<hex-bytes>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <root.omg>";

pub(super) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Option<RunArguments>, String> {
    let mut build_inputs = Vec::new();
    let mut both = false;
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
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some(flag @ ("--build-input" | "--optional-build-input")) => {
                build_inputs.push(super::parse_build_input(&mut arguments, flag)?);
            }
            Some("--help" | "-h") => super::set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--offline") => return Err("omega run does not support --offline".into()),
            Some("--both") => super::set_flag(&mut both, "--both")?,
            Some("--engine") => {
                if engine.is_some() {
                    return Err("duplicate --engine".into());
                }
                let value = super::option_value(&mut arguments)
                    .ok_or("--engine requires `native` or `terminal`")?;
                engine = Some(match value.to_str() {
                    Some("native") => ExecutionSelection::Native,
                    Some("terminal") => ExecutionSelection::Terminal,
                    _ => {
                        return Err(format!(
                            "--engine requires `native` or `terminal`, not `{}`",
                            value.to_string_lossy()
                        ));
                    }
                });
            }
            Some("--keep") => super::set_flag(&mut keep, "--keep")?,
            Some("--psi-work-budget") => {
                if psi_work_budget.is_some() {
                    return Err("duplicate --psi-work-budget".into());
                }
                let value = super::option_value(&mut arguments)
                    .ok_or("--psi-work-budget requires a positive u64")?
                    .into_string()
                    .map_err(|_| "--psi-work-budget requires a positive u64")?;
                psi_work_budget = Some(
                    value
                        .parse::<std::num::NonZeroU64>()
                        .map_err(|_| "--psi-work-budget requires a positive u64")?,
                );
            }
            Some("--entry-ram") => {
                let value = arguments
                    .next()
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
                private_entry_ram.push(compilation_products::PrivateEntryRamInitialization {
                    position,
                    contents,
                });
            }
            Some("--input") => {
                let file = super::option_value(&mut arguments).ok_or("--input requires a file")?;
                input_files.push(std::path::PathBuf::from(file));
            }
            Some(flag @ ("--inputs" | "--outputs")) => {
                let slot = if flag == "--inputs" {
                    &mut inputs
                } else {
                    &mut outputs
                };
                if slot.is_some() {
                    return Err(format!("duplicate {flag}"));
                }
                let directory = super::option_value(&mut arguments)
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
    let selection = match (engine, both) {
        (Some(_), true) => {
            return Err("--engine and --both are mutually exclusive: --both already runs native and Terminal".into());
        }
        (Some(engine), false) => engine,
        (None, true) => ExecutionSelection::Both,
        (None, false) => ExecutionSelection::Native,
    };
    let terminal = selection == ExecutionSelection::Terminal;
    let cases = inputs.is_some() || !input_files.is_empty();
    if inputs.is_some() && !input_files.is_empty() {
        return Err("use either --inputs <dir> or --input <file>, not both".into());
    }
    if cases != outputs.is_some() {
        return Err("--inputs or --input is used together with --outputs".into());
    }
    if cases && target_name.is_some() {
        return Err("case runs execute on the host; they cannot take --target".into());
    }
    if cases && terminal {
        return Err("case runs do not yet support --engine terminal; use --both".into());
    }
    if psi_work_budget.is_some() && !terminal && (!both || target_name.is_some()) {
        return Err("--psi-work-budget requires --engine terminal or --both host execution".into());
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
    Ok(Some(RunArguments {
        build_inputs,
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
    use crate::execution::ExecutionSelection;

    fn parse(arguments: &[&str]) -> Result<super::RunArguments, String> {
        parse_arguments(arguments.iter().map(std::ffi::OsString::from))?
            .ok_or_else(|| "unexpected --help".to_owned())
    }

    #[test]
    fn every_execution_route_retains_the_same_caller_source_inventory() {
        use package_compilation::BuildSourceCaptureObligation::{Optional, Required};
        for route in [
            vec!["--engine", "native"],
            vec!["--engine", "terminal"],
            vec!["--both"],
            vec!["--both", "--input", "case.stdin", "--outputs", "out"],
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
        let arguments = parse(&["--both", "--inputs", "data", "--outputs", "out", "main.omg"])
            .expect("case run parses");
        assert_eq!(arguments.selection, ExecutionSelection::Both);
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
        let arguments = parse(&["--psi-work-budget", "100000000", "--both", "main.omg"])
            .expect("positive host reference allowance parses");
        assert_eq!(
            arguments.psi_work_budget.map(std::num::NonZeroU64::get),
            Some(100_000_000)
        );
        assert!(
            parse(&["--both", "main.omg"])
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
                vec!["--engine", "terminal", "main.omg"],
                ExecutionSelection::Terminal,
            ),
            (
                vec!["main.omg", "--engine", "terminal"],
                ExecutionSelection::Terminal,
            ),
            (vec!["--both", "main.omg"], ExecutionSelection::Both),
            (
                vec![
                    "--engine",
                    "terminal",
                    "--target",
                    "linux_x86_64",
                    "main.omg",
                ],
                ExecutionSelection::Terminal,
            ),
            (
                vec!["--engine", "terminal", "--psi-work-budget", "5", "main.omg"],
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
            vec!["--engine", "terminal", "--both", "main.omg"],
            vec!["--both", "--engine", "native", "main.omg"],
            vec!["--engine", "jit", "main.omg"],
            vec!["--engine", "main.omg"],
            vec!["--engine", "native", "--engine", "terminal", "main.omg"],
            vec!["--engine", "native", "--psi-work-budget", "5", "main.omg"],
            vec![
                "--engine",
                "terminal",
                "--inputs",
                "d",
                "--outputs",
                "o",
                "main.omg",
            ],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?} should reject");
        }
    }

    #[test]
    fn psi_work_budget_rejects_invalid_or_unused_requests() {
        assert!(parse(&["--both", "--terminal-fuel-allowance", "1", "main.omg"]).is_err());
        for value in ["0", "-1", "many", "18446744073709551616"] {
            assert!(parse(&["--both", "--psi-work-budget", value, "main.omg"]).is_err());
        }
        for arguments in [
            vec!["--psi-work-budget", "--both", "main.omg"],
            vec!["--psi-work-budget", "1", "main.omg"],
            vec![
                "--both",
                "--target",
                "linux_x86_64",
                "--psi-work-budget",
                "1",
                "main.omg",
            ],
            vec![
                "--both",
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
