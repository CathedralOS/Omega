//! Parse one invocation before any compiler or package work begins.
//!
//! Every subcommand parser returns `Result<Option<T>, String>`: `Ok(Some(request))`
//! for a validated command, `Ok(None)` when `--help` or `-h` was requested, and
//! `Err(reason)` with the specific argument error before `finish_subcommand`
//! appends the command's usage text.

pub(crate) mod audit;
pub(crate) mod compile;
pub(crate) mod inspect;
pub(crate) mod packages;
pub(crate) mod run;

pub(crate) use audit::SourceArguments;
pub(crate) use compile::CompileArguments;
pub(crate) use inspect::InspectTerminalArguments;
pub(crate) use run::RunArguments;

use package_compilation::BuildSourceCaptureObligation;
use package_manager::operations::PackageInspectionOptions;
use package_manager::{PackageCommand, PackageCommandKind, PackageCommandOptions};
use std::ffi::OsString;

/// Exit status for an invocation the CLI cannot parse or whose argument values
/// it cannot accept, before any compiler or package work begins. It is BSD
/// `EX_USAGE`, outside 0-3: those are the compile command's compiler-request
/// halt values (`wiki/spec/build/compiler_request.md`), where 2 means
/// `Incomplete`.
pub(crate) const USAGE_ERROR_EXIT: u8 = 64;

pub(crate) enum Invocation {
    Compile(CompileArguments),
    Run(RunArguments),
    InspectTerminal(InspectTerminalArguments),
    Package {
        command: PackageCommand,
        options: PackageCommandOptions,
        accept_all: bool,
    },
    AuditSource(SourceArguments),
    AuditPackages(PackageInspectionOptions),
    Help(&'static str),
    Version,
}

pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Invocation, String> {
    let first = arguments.next();
    match first.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("install") => parse_package(PackageCommandKind::Install, arguments),
        Some("update") => parse_package(PackageCommandKind::Update, arguments),
        Some("audit") => parse_audit(arguments),
        Some("run") => {
            finish_subcommand(run::parse_arguments(arguments), run::USAGE, Invocation::Run)
        }
        Some("inspect-terminal") => finish_subcommand(
            inspect::parse_arguments(arguments),
            inspect::USAGE,
            Invocation::InspectTerminal,
        ),
        Some("--help" | "-h" | "help") => Ok(Invocation::Help(compile::USAGE)),
        Some("--version" | "-V") => {
            if arguments.next().is_some() {
                return Err("omega --version takes no arguments".to_owned());
            }
            Ok(Invocation::Version)
        }
        _ => finish_subcommand(
            compile::parse_arguments(first.into_iter().chain(arguments)),
            compile::USAGE,
            Invocation::Compile,
        ),
    }
}

fn finish_subcommand<T>(
    parsed: Result<Option<T>, String>,
    usage: &'static str,
    build: impl FnOnce(T) -> Invocation,
) -> Result<Invocation, String> {
    match parsed {
        Ok(Some(arguments)) => Ok(build(arguments)),
        Ok(None) => Ok(Invocation::Help(usage)),
        Err(error) => Err(format!("{error}\n{usage}")),
    }
}

fn parse_package(
    kind: PackageCommandKind,
    arguments: impl Iterator<Item = OsString>,
) -> Result<Invocation, String> {
    let usage = packages::usage(&kind);
    finish_subcommand(
        packages::parse_arguments(kind, arguments),
        usage,
        |(command, options, accept_all)| Invocation::Package {
            command,
            options,
            accept_all,
        },
    )
}

fn parse_audit(mut arguments: impl Iterator<Item = OsString>) -> Result<Invocation, String> {
    let subcommand = arguments.next();
    match subcommand.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("source") => finish_subcommand(
            audit::parse_source(arguments),
            audit::SOURCE_USAGE,
            Invocation::AuditSource,
        ),
        Some("packages") => finish_subcommand(
            audit::parse_packages(arguments),
            audit::PACKAGES_USAGE,
            Invocation::AuditPackages,
        ),
        Some("--help" | "-h") => Ok(Invocation::Help(audit::USAGE)),
        _ => {
            let error = subcommand.map_or_else(String::new, |subcommand| {
                format!("unknown audit command `{}`\n", subcommand.to_string_lossy())
            });
            Err(format!("{error}{}", audit::USAGE))
        }
    }
}

fn set_flag(value: &mut bool, flag: &str) -> Result<(), String> {
    if *value {
        return Err(format!("duplicate {flag}"));
    }
    *value = true;
    Ok(())
}

fn option_value(arguments: &mut impl Iterator<Item = OsString>) -> Option<OsString> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
}

fn take_value(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<OsString, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if value.is_empty() || value.as_encoded_bytes().starts_with(b"-") {
        return Err(format!("{flag} requires a nonempty value, not an option"));
    }
    Ok(value)
}

fn take_text(arguments: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<String, String> {
    take_value(arguments, flag)?
        .into_string()
        .map_err(|_| format!("{flag} requires a UTF-8 value"))
}

fn parse_build_input(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<(Vec<u8>, BuildSourceCaptureObligation), String> {
    let path = option_value(arguments)
        .ok_or_else(|| format!("{flag} requires a canonical relative path"))?
        .into_string()
        .map_err(|_| format!("{flag} requires a UTF-8 canonical relative path"))?;
    let obligation = if flag == "--build-input" {
        BuildSourceCaptureObligation::Required
    } else {
        BuildSourceCaptureObligation::Optional
    };
    Ok((path.into_bytes(), obligation))
}

#[cfg(test)]
mod tests {
    use super::{Invocation, parse};
    use std::ffi::OsString;

    fn parse_slice(arguments: &[&str]) -> Result<Invocation, String> {
        parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn help_succeeds_across_every_command_surface() {
        for invocation in [
            &["--help"][..],
            &["-h"],
            &["help"],
            &["--check", "--help"],
            &["run", "--help"],
            &["run", "-h"],
            &["inspect-terminal", "--help"],
            &["inspect-terminal", "-h"],
            &["install", "--help"],
            &["install", "-h"],
            &["update", "--help"],
            &["update", "-h"],
            &["audit", "--help"],
            &["audit", "-h"],
            &["audit", "source", "--help"],
            &["audit", "source", "-h"],
            &["audit", "packages", "--help"],
            &["audit", "packages", "-h"],
        ] {
            assert!(
                matches!(parse_slice(invocation), Ok(Invocation::Help(_))),
                "{invocation:?} should return Help"
            );
        }
    }

    #[test]
    fn inspect_and_audit_source_report_specific_errors() {
        let error = parse_slice(&["inspect-terminal", "--unknown"])
            .err()
            .unwrap();
        assert!(error.contains("unrecognized option `--unknown`"), "{error}");

        let error = parse_slice(&["audit", "source", "--kind", "local"])
            .err()
            .unwrap();
        assert!(error.contains("missing source locator"), "{error}");
    }
}
