// Parse arguments for the default `omega [path]` and `omega --check [path]` command.
//
// Repeated `--target` flags are deduplicated in first-seen order because the
// target selection is a set (`wiki/spec/build/configuration.md#exact-target-requests`).
// Any unrecognized `--` token is rejected before the positional `root_path`
// slot is assigned so a misspelled flag never masquerades as a missing source
// file.

use super::{option_value, parse_build_input};
use compiler::OptimizationRollback;
use package_compilation::BuildSourceCaptureRequest;
use std::path::PathBuf;

pub(crate) struct CompileArguments {
    pub(crate) build_dir: Option<PathBuf>,
    pub(crate) build_inputs: Option<BuildSourceCaptureRequest>,
    pub(crate) check_only: bool,
    pub(crate) verify: bool, // With --check, withhold the lock's accepted checkpoint to force fresh verification.
    pub(crate) offline: bool,
    pub(crate) timings: bool,
    pub(crate) report_file: Option<PathBuf>,
    pub(crate) installation_assumptions:
        Vec<compilation_products::InstallationEnvironmentAssumption>,
    pub(crate) root_path: PathBuf,
    pub(crate) target_names: Vec<String>, // Every --target named, in first-seen order, repeats removed; empty means host default.
    pub(crate) optimization_rollback: OptimizationRollback,
}

pub(super) const USAGE: &str = "usage: omega [--check] [--verify] [--offline] [--timings] [--report-file <path>] [--bootstrap-assumptions <review-record>] [--interrupt-delivery-assumptions <review-record>] [--build-dir <dir>] [--build-input <path>]... [--optional-build-input <path>]... [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>\n       omega run [--engine <native|psi> | --both [--psi-work-budget <positive-u64>]] [--keep] [--build-input <path>]... [--optional-build-input <path>]... [--entry-ram <position=hex>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <root.omg>\n       omega psi inspect --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega psi inspect --artifact <file>\n       omega source rewrite <ExactRule>... [--check] <root.omg>\n       omega packages audit --kind <local|git> <locator> [--rev <rev>]\n       omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega packages <install|update> --resume [--project <dir>] [--offline]\n       omega packages <install|update> --discard-review [--project <dir>] [--offline]\n       omega --version\n--verify (with --check) withholds the accepted lock's checkpoint and recorded trust admissions so the package re-verifies under retained source pins instead of recorded acceptance.\n--offline disables package source network acquisition for this invocation.\nrun and psi inspect do not support --offline.\nsource rewrite is specified and not implemented by this compiler.\n--entry-ram constructs checked private scalar-record RAM for an exact placed entry parameter; it supplies no host address or authority.\n--build-input and --optional-build-input select the exact root source inventory shared by Build and Product, relative to the source root.\nDirectories include their subtrees; required source files must be included explicitly.\n--bootstrap-assumptions and --interrupt-delivery-assumptions supply distinct exact-entry platform assumptions; neither proves physical truth.\n--report-file writes the produced compile report as a plain-text observation file at the named path.";

pub(super) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Option<CompileArguments>, String> {
    let mut build_dir = None;
    let mut build_inputs = Vec::new();
    let mut check_only = false;
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

    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            super::set_flag(&mut help, &argument.to_string_lossy())?;
            continue;
        }

        if argument == "--offline" {
            super::set_flag(&mut offline, "--offline")?;
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
            let path = option_value(&mut arguments)
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
            let rows = compilation_products::decode_installation_assumptions(&bytes)?;
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

        if argument == "--report-file" {
            if report_file.is_some() {
                return Err("duplicate --report-file".into());
            }
            report_file = option_value(&mut arguments).map(PathBuf::from);
            if report_file.is_none() {
                return Err("--report-file requires a path".into());
            }
            continue;
        }

        if argument == "--build-dir" {
            build_dir = option_value(&mut arguments).map(PathBuf::from);
            if build_dir.is_none() {
                return Err("--build-dir requires a directory".into());
            }
            continue;
        }

        if argument == "--target" {
            let target_name = option_value(&mut arguments)
                .and_then(|target_name| target_name.into_string().ok())
                .ok_or_else(|| "--target requires a UTF-8 target name".to_owned())?;
            if !target_names.contains(&target_name) {
                target_names.push(target_name);
            }
            continue;
        }

        if argument == "--build-input" || argument == "--optional-build-input" {
            build_inputs.push(parse_build_input(
                &mut arguments,
                &argument.to_string_lossy(),
            )?);
            continue;
        }

        if argument == "--disable-optimization" {
            let Some(name) = option_value(&mut arguments) else {
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
    if verify && !check_only {
        return Err("--verify requires --check".into());
    }
    if help {
        return Ok(None);
    }
    Ok(Some(CompileArguments {
        build_dir,
        build_inputs,
        check_only,
        verify,
        offline,
        timings,
        report_file,
        installation_assumptions: bootstrap_assumptions
            .unwrap_or_default()
            .into_iter()
            .chain(delivery_assumptions.unwrap_or_default())
            .collect(),
        root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
        target_names,
        optimization_rollback,
    }))
}
