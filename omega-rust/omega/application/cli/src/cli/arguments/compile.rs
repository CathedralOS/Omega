use super::{option_value, parse_build_input};
use compiler::OptimizationRollback;
use package_compilation::BuildSourceCaptureRequest;
use std::path::PathBuf;

pub(crate) struct CompileArguments {
    pub(crate) build_dir: Option<PathBuf>,
    pub(crate) build_inputs: Option<BuildSourceCaptureRequest>,
    pub(crate) check_only: bool,
    /// With `--check`, withhold the lock's accepted checkpoint so the root
    /// compiles under fresh verification rather than retained acceptance.
    pub(crate) verify: bool,
    pub(crate) offline: bool,
    pub(crate) timings: bool,
    pub(crate) report_file: Option<PathBuf>,
    pub(crate) root_path: PathBuf,
    /// Every `--target` named, in the order given, repeats removed. Empty
    /// means none was named.
    pub(crate) target_names: Vec<String>,
    pub(crate) optimization_rollback: OptimizationRollback,
}

pub(crate) fn usage() -> &'static str {
    "usage: omega [--check] [--verify] [--offline] [--timings] [--report-file <path>] [--build-dir <dir>] [--build-input <path>]... [--optional-build-input <path>]... [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>\n       omega run [--both] [--keep] [--entry-ram <position=hex>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega inspect-terminal --artifact <file>\n       omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\n       omega install|update --resume [--project <dir>] [--offline]\n       omega install|update --discard-review [--project <dir>] [--offline]\n       omega --version\n--verify (with --check) withholds the accepted lock's checkpoint and recorded trust admissions so the package re-verifies under retained source pins instead of recorded acceptance.\n--offline disables package source network acquisition for this invocation.\nrun and inspect-terminal do not support --offline.\n--entry-ram constructs checked private scalar-record RAM for an exact placed entry parameter; it supplies no host address or authority.\n--build-input and --optional-build-input select the exact build input inventory relative to the source root.\nDirectories include their subtrees; required source files must be included explicitly.\n--report-file writes the produced compile report as a plain-text observation file at the named path."
}

pub(crate) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<CompileArguments, String> {
    let mut build_dir = None;
    let mut build_inputs = Vec::new();
    let mut check_only = false;
    let mut disabled_optimizations = Vec::new();
    let mut offline = false;
    let mut timings = false;
    let mut verify = false;
    let mut report_file = None;
    let mut root_path = None;
    let mut target_names: Vec<String> = Vec::new();

    while let Some(argument) = arguments.next() {
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
            // The selection is a set: naming a target twice adds nothing
            // (wiki/spec/build/configuration.md#exact-target-requests).
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

        // Falling through would assign this as the root path, so a misspelled flag
        // surfaced as a package-resolution failure against a directory that does not
        // exist, rather than as a rejected option.
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
    if verify && !check_only {
        return Err("--verify requires --check".into());
    }
    Ok(CompileArguments {
        build_dir,
        build_inputs,
        check_only,
        verify,
        offline,
        timings,
        report_file,
        root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
        target_names,
        optimization_rollback,
    })
}
