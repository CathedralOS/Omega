/*

Command-line parser for the `omega` binary.

We parse the entire invocation into an `Invocation` value before any compiler
or package work starts. The first argument selects the command surface
(`packages`, `psi`, `source`, `run`, `--help`, or `--version`); anything else
is passed to `compile::parse_arguments` with that first token put back at the
front of the iterator, because `omega <root.omg>` and `omega --check <root.omg>`
are the default command. Flat `install`/`update`, action-first `audit`, and
`inspect-terminal` are rejected. They are not aliases of the subsystem forms
in `wiki/spec/build/command_line.md`.

Every subcommand parser in `compile`, `run`, `inspect`, `packages`, and `audit`
returns `Result<Option<T>, String>`:
- `Ok(Some(request))` for a complete, validated request.
- `Ok(None)` when `--help` or `-h` was seen anywhere in the argument list.
- `Err(reason)` with the specific problem; `finish_subcommand` appends that
  subcommand's `USAGE` string so the caller gets both the exact mistake and the
  synopsis.

Shared value readers (`option_value`, `take_value`, `take_text`) refuse tokens
that start with `-` or `--`. That way, if a user forgets the path after
`--input` or `--report-file` and writes `--input --both`, we fail immediately
on `--input` instead of quietly swallowing `--both` as a file name.

*/

/*

Why `USAGE_ERROR_EXIT` is 64:

Exit codes 0 through 3 are reserved by the compiler request contract
(`wiki/spec/build/compiler_request.md`), where 0 is `Accept`, 1 is `Reject`,
and 2 is `Incomplete` (a proof search that hit a resource bound without
refuting the program). If argument errors exited with 1 or 2, a mistyped flag
in a test harness or build script would be indistinguishable from a rejected or
incomplete program. We use BSD `EX_USAGE` (64) so CLI usage mistakes stay
disjoint from compiler verdicts.

*/

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

const SOURCE_REWRITE_USAGE: &str = "usage: omega source rewrite <ExactRule>... [--check] <root.omg>\nSource rewriting is specified and not implemented by this compiler.";

pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Invocation, String> {
    let first = arguments.next();
    match first.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("packages") => parse_packages_subsystem(arguments),
        Some("psi") => parse_psi_subsystem(arguments),
        Some("source") => parse_source_subsystem(arguments),
        Some("run") => {
            finish_subcommand(run::parse_arguments(arguments), run::USAGE, Invocation::Run)
        }
        Some(retired @ ("install" | "update" | "audit" | "inspect-terminal")) => {
            Err(retired_command(retired))
        }
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

fn retired_command(spelling: &str) -> String {
    let command = match spelling {
        "install" => "omega packages install",
        "update" => "omega packages update",
        "audit" => "omega packages audit",
        "inspect-terminal" => "omega psi inspect",
        _ => "omega --help",
    };
    format!(
        "`{spelling}` is retired. The command is `{command}`.\n{USAGE}",
        USAGE = compile::USAGE
    )
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

fn parse_packages_subsystem(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Invocation, String> {
    let action = arguments.next();
    match action.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("install") => parse_package(PackageCommandKind::Install, arguments),
        Some("update") => parse_package(PackageCommandKind::Update, arguments),
        Some("audit") => {
            finish_subcommand(
                audit::parse(arguments),
                audit::USAGE,
                |request| match request {
                    audit::ParsedAudit::Locator(arguments) => Invocation::AuditSource(arguments),
                    audit::ParsedAudit::Project(options) => Invocation::AuditPackages(options),
                },
            )
        }
        Some("--help" | "-h") => Ok(Invocation::Help(packages::USAGE)),
        _ => {
            let error = action.map_or_else(
                || "missing packages command\n".to_owned(),
                |action| format!("unknown packages command `{}`\n", action.to_string_lossy()),
            );
            Err(format!("{error}{}", packages::USAGE))
        }
    }
}

fn parse_psi_subsystem(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Invocation, String> {
    let action = arguments.next();
    match action.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("inspect") => finish_subcommand(
            inspect::parse_arguments(arguments),
            inspect::USAGE,
            Invocation::InspectTerminal,
        ),
        Some("--help" | "-h") => Ok(Invocation::Help(inspect::USAGE)),
        _ => {
            let error = action.map_or_else(
                || "missing psi command\n".to_owned(),
                |action| format!("unknown psi command `{}`\n", action.to_string_lossy()),
            );
            Err(format!("{error}{}", inspect::USAGE))
        }
    }
}

fn parse_source_subsystem(arguments: impl Iterator<Item = OsString>) -> Result<Invocation, String> {
    let mut help = false;
    let mut action = None;
    for argument in arguments {
        if argument == "--help" || argument == "-h" {
            help = true;
            continue;
        }
        if action.is_none() && !argument.as_encoded_bytes().starts_with(b"-") {
            action = Some(argument);
        }
    }
    if help {
        return Ok(Invocation::Help(SOURCE_REWRITE_USAGE));
    }
    match action.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("rewrite") => Err(format!(
            "omega source rewrite is not implemented\n{SOURCE_REWRITE_USAGE}"
        )),
        Some(other) => Err(format!(
            "unknown source command `{other}`\n{SOURCE_REWRITE_USAGE}"
        )),
        None => Err(format!("missing source command\n{SOURCE_REWRITE_USAGE}")),
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
    use super::{InspectTerminalArguments, Invocation, parse};
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
            &["psi", "--help"],
            &["psi", "-h"],
            &["psi", "inspect", "--help"],
            &["psi", "inspect", "-h"],
            &["packages", "--help"],
            &["packages", "-h"],
            &["packages", "install", "--help"],
            &["packages", "install", "-h"],
            &["packages", "update", "--help"],
            &["packages", "update", "-h"],
            &["packages", "audit", "--help"],
            &["packages", "audit", "-h"],
            &["source", "--help"],
            &["source", "rewrite", "--help"],
            &["source", "rewrite", "-h"],
        ] {
            assert!(
                matches!(parse_slice(invocation), Ok(Invocation::Help(_))),
                "{invocation:?} should return Help"
            );
        }
    }

    #[test]
    fn spec_spellings_parse() {
        match parse_slice(&["packages", "install", "lib", "--as", "leaf"]).unwrap() {
            Invocation::Package { command, .. } => match command {
                package_manager::PackageCommand::Install { source, alias, .. } => {
                    assert_eq!(source, "lib");
                    assert_eq!(alias.as_deref(), Some("leaf"));
                }
                _ => panic!("install did not parse as a package install"),
            },
            _ => panic!("install did not parse as a package command"),
        }
        match parse_slice(&["packages", "update", "leaf", "--to", "abc"]).unwrap() {
            Invocation::Package { command, .. } => match command {
                package_manager::PackageCommand::Update { packages, revision } => {
                    assert_eq!(packages, ["leaf"]);
                    assert_eq!(revision.as_deref(), Some("abc"));
                }
                _ => panic!("update did not parse as a package update"),
            },
            _ => panic!("update did not parse as a package command"),
        }
        match parse_slice(&["packages", "update", "--resume"]).unwrap() {
            Invocation::Package { command, .. } => {
                assert!(matches!(
                    command,
                    package_manager::PackageCommand::Resume {
                        kind: package_manager::PackageCommandKind::Update,
                    }
                ));
            }
            _ => panic!("resume did not parse as a package command"),
        }
        match parse_slice(&["packages", "audit", "--project", "demo", "--details"]).unwrap() {
            Invocation::AuditPackages(options) => {
                assert_eq!(options.project_root, std::path::PathBuf::from("demo"));
                assert!(options.details);
            }
            _ => panic!("project audit did not parse"),
        }
        match parse_slice(&["packages", "audit", "--kind", "git", "src", "--rev", "main"]).unwrap()
        {
            Invocation::AuditSource(arguments) => {
                assert_eq!(arguments.source_kind, "git");
                assert_eq!(arguments.locator, "src");
                assert_eq!(arguments.rev.as_deref(), Some("main"));
            }
            _ => panic!("locator audit did not parse"),
        }
        match parse_slice(&["psi", "inspect", "--machine", "main", "root.omg"]).unwrap() {
            Invocation::InspectTerminal(InspectTerminalArguments::Source {
                machine,
                root_path,
                ..
            }) => {
                assert_eq!(machine, "main");
                assert_eq!(root_path, std::path::PathBuf::from("root.omg"));
            }
            _ => panic!("source inspection did not parse"),
        }
        match parse_slice(&["psi", "inspect", "--artifact", "program.psi"]).unwrap() {
            Invocation::InspectTerminal(InspectTerminalArguments::Artifact { path }) => {
                assert_eq!(path, std::path::PathBuf::from("program.psi"));
            }
            _ => panic!("artifact inspection did not parse"),
        }
        match parse_slice(&["run", "--engine", "psi", "main.omg"]).unwrap() {
            Invocation::Run(arguments) => {
                assert_eq!(
                    arguments.selection,
                    crate::execution::ExecutionSelection::Terminal
                );
            }
            _ => panic!("psi engine did not parse"),
        }
    }

    #[test]
    fn retired_spellings_are_rejected() {
        for invocation in [
            &["install", "lib"][..],
            &["install", "--help"],
            &["update"],
            &["update", "--resume"],
            &["audit"],
            &["audit", "packages", "--details"],
            &["audit", "source", "--kind", "local", "src"],
            &["inspect-terminal", "--artifact", "program.psi"],
            &["inspect-terminal", "--help"],
            &["run", "--engine", "terminal", "main.omg"],
            &["psi", "inspect-terminal", "--artifact", "program.psi"],
        ] {
            let Err(error) = parse_slice(invocation) else {
                panic!("{invocation:?} should reject");
            };
            assert!(
                error.contains("retired")
                    || error.contains("requires `native` or `psi`")
                    || error.contains("unknown psi command"),
                "{invocation:?} rejected with {error}"
            );
        }
    }

    #[test]
    fn package_audit_rejects_mixed_forms_and_unknown_kinds() {
        for invocation in [
            &["packages", "audit", "--kind", "local", "src", "--details"][..],
            &[
                "packages",
                "audit",
                "--kind",
                "local",
                "src",
                "--project",
                "demo",
            ],
            &["packages", "audit", "--rev", "main"],
            &["packages", "audit", "--kind", "ftp", "src"],
            &["packages", "audit", "src"],
        ] {
            assert!(
                parse_slice(invocation).is_err(),
                "{invocation:?} should reject"
            );
        }
        let error = parse_slice(&["packages", "audit", "--kind", "local"])
            .err()
            .unwrap();
        assert!(error.contains("missing source locator"), "{error}");
    }

    #[test]
    fn source_rewrite_is_recognized_and_not_implemented() {
        let error = parse_slice(&["source", "rewrite", "redundant-call-markers", "main.omg"])
            .err()
            .unwrap();
        assert!(error.contains("not implemented"), "{error}");
        assert!(error.contains("omega source rewrite"), "{error}");
        let error = parse_slice(&["source", "format", "main.omg"])
            .err()
            .unwrap();
        assert!(error.contains("unknown source command"), "{error}");
    }

    #[test]
    fn inspect_reports_specific_errors() {
        let error = parse_slice(&["psi", "inspect", "--unknown"]).err().unwrap();
        assert!(error.contains("unrecognized option `--unknown`"), "{error}");
    }
}
