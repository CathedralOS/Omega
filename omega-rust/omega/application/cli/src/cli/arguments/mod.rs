//! Parse one invocation before any compiler or package work begins.

mod compile;
mod execution;
mod inspection;
mod package_audit;
mod packages;
mod source_audit;

pub(crate) use compile::CompileArguments;
pub(crate) use execution::RunArguments;
pub(crate) use inspection::InspectTerminalArguments;
pub(crate) use source_audit::SourceArguments;

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
        Some("run") => execution::parse_arguments(arguments)
            .map(Invocation::Run)
            .map_err(|error| {
                format!("{error}\nusage: omega run [--both] [--keep] [--entry-ram <position=hex>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <main.omg>")
            }),
        Some("inspect-terminal") => inspection::parse_inspect_terminal_arguments(arguments)
            .map(Invocation::InspectTerminal)
            .ok_or_else(|| {
                "usage: omega inspect-terminal --machine <qualified> [--target <name>] [--output <file>] <root.omg>\n       omega inspect-terminal --artifact <file>"
                    .to_owned()
            }),
        // The usage text already exists and every other entry point prints
        // it on a parse failure; without this arm the conventional spelling
        // answers "unrecognized option `--help`" instead of showing it.
        Some("--help" | "-h" | "help") => Ok(Invocation::Help(compile::usage())),
        Some("--version" | "-V") => {
            if arguments.next().is_some() {
                return Err("omega --version takes no arguments".to_owned());
            }
            Ok(Invocation::Version)
        }
        _ => compile::parse_arguments(first.into_iter().chain(arguments))
            .map(Invocation::Compile)
            .map_err(|error| format!("{error}\n{}", compile::usage())),
    }
}

fn parse_package(
    kind: PackageCommandKind,
    arguments: impl Iterator<Item = OsString>,
) -> Result<Invocation, String> {
    let usage = packages::usage(&kind);
    match packages::parse_arguments(kind, arguments) {
        Ok(Some((command, options, accept_all))) => Ok(Invocation::Package {
            command,
            options,
            accept_all,
        }),
        Ok(None) => Ok(Invocation::Help(usage)),
        Err(error) => Err(format!("{error}\n{usage}")),
    }
}

fn parse_audit(mut arguments: impl Iterator<Item = OsString>) -> Result<Invocation, String> {
    const SOURCE_USAGE: &str =
        "usage: omega audit source --kind <local|git> <locator> [--rev <rev>]";
    let subcommand = arguments.next();
    match subcommand.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("source") => source_audit::parse_source_arguments(arguments)
            .map(Invocation::AuditSource)
            .ok_or_else(|| SOURCE_USAGE.to_owned()),
        Some("packages") => match package_audit::parse(arguments) {
            Ok(Some(options)) => Ok(Invocation::AuditPackages(options)),
            Ok(None) => Ok(Invocation::Help(package_audit::USAGE)),
            Err(error) => Err(format!("{error}\n{}", package_audit::USAGE)),
        },
        _ => {
            let error = subcommand.map_or_else(String::new, |subcommand| {
                format!("unknown audit command `{}`\n", subcommand.to_string_lossy())
            });
            Err(format!("{error}{SOURCE_USAGE}\n{}", package_audit::USAGE))
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
