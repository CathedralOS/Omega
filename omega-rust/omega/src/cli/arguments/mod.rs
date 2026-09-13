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

use package_manager::operations::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageInspectionOptions,
};
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) enum Invocation {
    Compile(CompileArguments),
    Run(RunArguments),
    InspectTerminal(InspectTerminalArguments),
    Package {
        command: PackageCommand,
        options: PackageCommandOptions,
    },
    AuditSource(SourceArguments),
    AuditPackages(PackageInspectionOptions),
    RefreshSamples(PathBuf),
    Help(&'static str),
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
                format!("{error}\nusage: omega run [--both] [--keep] [--target <name>] <main.omg>")
            }),
        Some("inspect-terminal") => inspection::parse_inspect_terminal_arguments(arguments)
            .map(Invocation::InspectTerminal)
            .ok_or_else(|| {
                "usage: omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>"
                    .to_owned()
            }),
        Some("refresh-samples") => Ok(Invocation::RefreshSamples(
            arguments
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("samples")),
        )),
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
        Ok(Some((command, options))) => Ok(Invocation::Package { command, options }),
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

fn option_value(arguments: &mut impl Iterator<Item = OsString>) -> Option<OsString> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
}

#[cfg(test)]
mod tests;
