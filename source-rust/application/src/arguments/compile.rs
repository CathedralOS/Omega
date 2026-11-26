// Parse arguments for the default `omega [path]` and `omega --check [path]` command.
//
// Repeated `--target` flags are deduplicated in first-seen order because the
// target selection is a set (`wiki/spec/build/configuration.md#exact-target-requests`).
// Any unrecognized `--` token is rejected before the positional `root_path`
// slot is assigned so a misspelled flag never masquerades as a missing source
// file.

use super::{option_value, parse_build_input};
use compiler::stage_validators::StageValidatorRequest;
use optimization_requests::OptimizationRollback;
use package_compilation::BuildSourceCaptureRequest;
use std::path::PathBuf;

pub(crate) struct CompileArguments {
    pub(crate) build_dir: Option<PathBuf>,
    pub(crate) build_inputs: Option<BuildSourceCaptureRequest>,
    pub(crate) check_only: bool,
    pub(crate) compare_tests: bool,
    pub(crate) emit_psi: Option<PathBuf>,
    pub(crate) trusted_artifact: Option<String>,
    pub(crate) offline: bool,
    pub(crate) timings: bool,
    pub(crate) report_file: Option<PathBuf>,
    pub(crate) installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
    pub(crate) root_path: PathBuf,
    pub(crate) target_names: Vec<String>, // Every --target named, in first-seen order, repeats removed; empty means host default.
    pub(crate) optimization_rollback: OptimizationRollback,
    pub(crate) stage_validators: StageValidatorRequest, // Empty unless --validator or --every-validator.
}

pub(super) const USAGE: &str = concat!(
    "usage: omega [--check] [--verify] [--offline] [--timings] [--report-file <path>] [--bootstrap-assumptions <review-record>] [--interrupt-delivery-assumptions <review-record>] [--build-dir <dir>] [--build-input <path>]... [--optional-build-input <path>]... [--target <name>]... [--disable-optimization <ExactName>]... [--validator <ExactName>]... [--every-validator] <root.omg>\n       ",
    "omega build [--compare-tests] <root.omg>\n       omega --emit-psi <program.psi> [--target <name>]... <root.omg>\n       omega --trusted-artifact <manifest-id> [--target <name>] [--build-dir <dir>] <program.psi>\n       ",
    super::run::run_help!(synopsis),
    "\n       omega psi inspect --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega psi inspect --artifact <file>\n       omega source parse [--] <file>\n       omega source rewrite <ExactRule>... [--check] <root.omg>\n       omega packages audit --kind <local|git> <locator> [--rev <rev>]\n       omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages <install|update> --resume [--project <dir>] [--offline]\n       omega packages <install|update> --discard-review [--project <dir>] [--offline]\n       omega --version\n--compare-tests runs every enabled registered test on both engines and compares their observations.\n--verify requires --check. Source checking re-verifies the captured package sources under their locked pins.\n--offline disables package source network acquisition for this invocation.\nrun and psi inspect do not support --offline.\nFor omega run, an explicit --target makes the default engine, --engine native,\nand --both emit artifacts without running the program, even for this host.\nWith --engine psi, --target selects the target environment for Psi execution.\n",
    super::run::run_help!(work_budget),
    "\n",
    super::run::run_help!(host_packages),
    "\nsource rewrite is specified and not implemented by this compiler.\n--entry-ram initializes a borrowed scalar-record argument declared by the installed entry contract. Hosted ProgramEntry declares no such argument.\n--build-input and --optional-build-input select the exact root source inventory shared by Build and Product, relative to the source root.\nDirectories include their subtrees; required source files must be included explicitly.\n--bootstrap-assumptions and --interrupt-delivery-assumptions supply distinct exact-entry platform assumptions; neither proves physical truth.\n--report-file writes the produced compile report as a plain-text observation file at the named path.\n--validator runs one named stage-output check inside this compilation; repeat it for more. SelectedInstructionRewriteReplay checks producer repeatability, not independent rewrite correctness. --every-validator runs every applicable registered validator and reports the rest as inapplicable. Default compilation runs none.\n--verbose, with any command, lists every error a failure reports; by default each cause of errors prints once with how many more share it.",
);

pub(super) fn parse_arguments(
    arguments: &mut super::Arguments,
) -> Result<Option<CompileArguments>, String> {
    let mut build_dir = None;
    let mut build_inputs = Vec::new();
    let mut check_only = false;
    let mut compare_tests = false;
    let mut emit_psi = None;
    let mut trusted_artifact = None;
    let mut disabled_optimizations = Vec::new();
    let mut help = false;
    let mut offline = false;
    let mut timings = false;
    let mut verify = false;
    let mut report_file = None;
    let mut bootstrap_assumptions = None;
    let mut delivery_assumptions = None;
    let mut root_path = None;
    let mut target_names: Vec<String> = Vec::new();
    let mut validator_names: Vec<String> = Vec::new();
    let mut every_validator = false;

    while let Some(argument) = arguments.next_argument() {
        if argument == "--help" || argument == "-h" {
            super::set_flag(&mut help, &argument.to_string_lossy())?;
            continue;
        }

        if argument == "--offline" {
            super::set_flag(&mut offline, "--offline")?;
            continue;
        }

        if argument == "--compare-tests" {
            super::set_flag(&mut compare_tests, "--compare-tests")?;
            continue;
        }

        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--verify" {
            super::set_flag(&mut verify, "--verify")?;
            continue;
        }

        if argument == "--timings" {
            super::set_flag(&mut timings, "--timings")?;
            continue;
        }

        if argument == "--bootstrap-assumptions" || argument == "--interrupt-delivery-assumptions" {
            let delivery = argument == "--interrupt-delivery-assumptions";
            let option_name = if delivery {
                "--interrupt-delivery-assumptions"
            } else {
                "--bootstrap-assumptions"
            };
            let destination = if delivery {
                &mut delivery_assumptions
            } else {
                &mut bootstrap_assumptions
            };
            if destination.is_some() {
                return Err(format!("duplicate {option_name}"));
            }
            let path = option_value(arguments)
                .map(PathBuf::from)
                .ok_or("installation assumption option requires a review record path")?;
            let metadata = std::fs::metadata(&path).map_err(|error| {
                format!(
                    "cannot inspect installation assumption record {}: {error}",
                    path.display()
                )
            })?;
            if metadata.len() > 1024 * 1024 {
                return Err("installation assumption record exceeds 1 MiB".into());
            }
            let bytes = std::fs::read(&path).map_err(|error| {
                format!(
                    "cannot read installation assumption record {}: {error}",
                    path.display()
                )
            })?;
            let rows = installation_conditions::decode_installation_assumptions(&bytes)?;
            if rows
                .iter()
                .any(|row| row.interrupt_delivery().is_some() != delivery)
            {
                return Err(format!(
                    "{option_name} contains another kind of environment condition"
                ));
            }
            *destination = Some(rows);
            continue;
        }

        if argument == "--emit-psi" {
            if emit_psi.is_some() {
                return Err("duplicate --emit-psi".into());
            }
            let path = PathBuf::from(super::take_value(arguments, "--emit-psi")?);
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
            {
                return Err("--emit-psi requires a .psi output file".into());
            }
            emit_psi = Some(path);
            continue;
        }
        if argument == "--trusted-artifact" {
            super::parse_trusted_artifact(arguments, &mut trusted_artifact)?;
            continue;
        }
        if argument == "--report-file" {
            if report_file.is_some() {
                return Err("duplicate --report-file".into());
            }
            report_file = option_value(arguments).map(PathBuf::from);
            if report_file.is_none() {
                return Err("--report-file requires a path".into());
            }
            continue;
        }

        if argument == "--build-dir" {
            if build_dir.is_some() {
                return Err("duplicate --build-dir".into());
            }
            build_dir = option_value(arguments).map(PathBuf::from);
            if build_dir.is_none() {
                return Err("--build-dir requires a directory".into());
            }
            continue;
        }

        if argument == "--target" {
            let target_name = option_value(arguments)
                .and_then(|target_name| target_name.into_string().ok())
                .ok_or_else(|| "--target requires a UTF-8 target name".to_owned())?;
            if !target_names.contains(&target_name) {
                target_names.push(target_name);
            }
            continue;
        }

        if argument == "--build-input" || argument == "--optional-build-input" {
            build_inputs.push(parse_build_input(arguments, &argument.to_string_lossy())?);
            continue;
        }

        if argument == "--validator" {
            let name = option_value(arguments)
                .and_then(|name| name.into_string().ok())
                .ok_or_else(|| "--validator requires one UTF-8 exact validator name".to_owned())?;
            validator_names.push(name);
            continue;
        }

        if argument == "--every-validator" {
            super::set_flag(&mut every_validator, "--every-validator")?;
            continue;
        }

        if argument == "--disable-optimization" {
            let Some(name) = option_value(arguments) else {
                return Err("--disable-optimization requires one exact optimization name".into());
            };
            let name = name.into_string().map_err(|_| {
                "--disable-optimization requires a UTF-8 exact optimization name".to_owned()
            })?;
            disabled_optimizations.push(name);
            continue;
        }

        if argument.to_string_lossy().starts_with("--") {
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

    let optimization_rollback =
        OptimizationRollback::from_exact_names(disabled_optimizations.iter().map(String::as_str))
            .map_err(|error| error.to_string())?;
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(BuildSourceCaptureRequest::new(build_inputs)?)
    };
    if check_only && (bootstrap_assumptions.is_some() || delivery_assumptions.is_some()) {
        return Err("installation assumptions apply only to native compilation".into());
    }
    let stage_validators = StageValidatorRequest::from_exact_names(
        validator_names.iter().map(String::as_str),
        every_validator,
    )?;
    if verify && !check_only {
        return Err("--verify requires --check".into());
    }
    if help {
        return Ok(None);
    }
    let root_path = root_path.ok_or_else(|| "missing root Omega source or Psi path".to_owned())?;
    let saved = root_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"));
    if compare_tests && (check_only || emit_psi.is_some() || saved) {
        return Err("--compare-tests requires a source build that runs its registered tests".into());
    }
    if trusted_artifact.is_some() && !saved {
        return Err("--trusted-artifact applies to a saved .psi program".into());
    }
    if saved
        && (emit_psi.is_some()
            || check_only
            || verify
            || offline
            || timings
            || report_file.is_some()
            || build_inputs.is_some()
            || !stage_validators.is_empty()
            || !optimization_rollback.is_empty()
            || bootstrap_assumptions.is_some()
            || delivery_assumptions.is_some())
    {
        return Err(
            "saved Psi compilation accepts --trusted-artifact, --target and --build-dir".into(),
        );
    }
    if emit_psi.is_some()
        && (check_only || bootstrap_assumptions.is_some() || delivery_assumptions.is_some())
    {
        return Err("--emit-psi requests a portable product and cannot take native installation assumptions or --check".into());
    }
    if emit_psi.is_some() && build_dir.is_some() {
        return Err(
            "--emit-psi saves one file for one source compilation; supply the output path directly"
                .into(),
        );
    }
    Ok(Some(CompileArguments {
        emit_psi,
        trusted_artifact,
        build_dir,
        build_inputs,
        check_only,
        compare_tests,
        offline,
        timings,
        report_file,
        installation_assumptions: bootstrap_assumptions
            .unwrap_or_default()
            .into_iter()
            .chain(delivery_assumptions.unwrap_or_default())
            .collect(),
        root_path,
        target_names,
        optimization_rollback,
        stage_validators,
    }))
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;
    use crate::arguments::Arguments;
    use std::ffi::OsString;

    #[test]
    fn repeating_build_dir_is_a_usage_error_even_for_the_same_directory() {
        for second in ["first", "second"] {
            let mut arguments = Arguments::new(
                ["--build-dir", "first", "--build-dir", second, "main.omg"]
                    .into_iter()
                    .map(OsString::from),
            );
            assert_eq!(
                parse_arguments(&mut arguments).err().as_deref(),
                Some("duplicate --build-dir")
            );
        }
    }
}
