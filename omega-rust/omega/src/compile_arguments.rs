use compiler::OptimizationRollback;
use std::path::PathBuf;

#[cfg(test)]
mod tests;

pub(crate) struct CompileArguments {
    pub(crate) accept_admissions: bool,
    pub(crate) build_dir: Option<PathBuf>,
    pub(crate) check_only: bool,
    pub(crate) offline: bool,
    pub(crate) output_only: bool,
    pub(crate) root_path: PathBuf,
    pub(crate) target_name: Option<String>,
    pub(crate) optimization_rollback: OptimizationRollback,
}

pub(crate) fn usage() -> &'static str {
    "usage: omega [--check] [--offline] [--accept-admissions] [--output-only] [--build-dir <dir>] [--target <name>] [--disable-optimization <ExactName>]... <root.omg>\n       omega run [--both] [--keep] [--target <name>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>\n       omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline]\n       omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline]\n       omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline]\n       omega install|update --resume [--project <dir>] [--offline]\n       omega install|update --discard-review [--project <dir>] [--offline]\n       omega refresh-samples [samples-dir]\n--offline disables package source network acquisition for this invocation.\nrun and inspect-terminal do not support --offline."
}

pub(crate) fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<CompileArguments, String> {
    let mut accept_admissions = false;
    let mut build_dir = None;
    let mut check_only = false;
    let mut disabled_optimizations = Vec::new();
    let mut offline = false;
    let mut output_only = false;
    let mut root_path = None;
    let mut target_name = None;

    while let Some(argument) = arguments.next() {
        if argument == "--offline" {
            if offline {
                return Err("duplicate --offline".into());
            }
            offline = true;
            continue;
        }

        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--accept-admissions" {
            accept_admissions = true;
            continue;
        }

        if argument == "--output-only" {
            output_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = compile_option_value(&mut arguments).map(PathBuf::from);
            if build_dir.is_none() {
                return Err("--build-dir requires a directory".into());
            }
            continue;
        }

        if argument == "--target" {
            target_name = compile_option_value(&mut arguments)
                .and_then(|target_name| target_name.into_string().ok());
            if target_name.is_none() {
                return Err("--target requires a UTF-8 target name".into());
            }
            continue;
        }

        if argument == "--disable-optimization" {
            let Some(name) = compile_option_value(&mut arguments) else {
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
    Ok(CompileArguments {
        accept_admissions,
        build_dir,
        check_only,
        offline,
        output_only,
        root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
        target_name,
        optimization_rollback,
    })
}

pub(crate) fn compile_option_value(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
}
