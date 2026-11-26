/*

Command-line parser for the `omega` binary.

We parse the entire invocation into an `Invocation` value before any compiler
or package work starts. After any global flags, the first argument selects
the command surface (`packages`, `psi`, `source`, `build`, `run`, `--help`, or
`--version`). Any other first token goes back on the argument list for
`compile::parse_arguments`, because `omega <root.omg>` and
`omega --check <root.omg>` are the default command. Flat `install`/`update`, action-first `audit`, and
`inspect-terminal` are rejected. They are not aliases of the subsystem forms
in `wiki/spec/build/command_line.md`.

Every subcommand parser in `compile`, `run`, `inspect`, `packages`, and `audit`
returns `Result<Option<T>, String>`:
- `Ok(Some(request))` for a complete, validated request.
- `Ok(None)` when `--help` or `-h` was seen anywhere in the argument list.
- `Err(reason)` with the specific problem; `finish_subcommand` appends that
  subcommand's `USAGE` string so the caller gets both the exact mistake and the
  synopsis.

A path read by `option_value` can start with a single dash. `take_value` and
its UTF-8 adapter `take_text` reject any leading dash. All three reject a
long option in the value position, so `--input --keep` reports the missing path.

*/

/*

Why `USAGE_ERROR_EXIT` is 64:

Exit codes 0 through 3 are reserved by the compiler request contract
(`wiki/spec/build/compiler_request.md`), where 0 is `Complete`, 1 is `Reject`,
2 is `Incomplete` (a proof search that hit a resource bound without
refuting the program), and 3 is `InternalFailure` (the compiler caught a defect
in itself). If argument errors exited with 1 through 3, a mistyped flag in a
test harness or build script would be indistinguishable from a compiler
verdict. We use BSD `EX_USAGE` (64) so CLI usage mistakes stay
disjoint from compiler verdicts.

*/

pub(crate) mod audit;
pub(crate) mod compile;
pub(crate) mod inspect;
pub(crate) mod packages;
mod programs;
pub(crate) mod run;
mod source;

pub(crate) use audit::SourceArguments;
pub(crate) use compile::CompileArguments;
pub(crate) use inspect::InspectTerminalArguments;
pub(crate) use run::RunArguments;

use crate::diagnostic_output::DiagnosticOutput;
use package_compilation::BuildSourceCaptureObligation;
use package_manager::operations::PackageInspectionOptions;
use package_manager::{PackageCommand, PackageCommandKind, PackageCommandOptions};
use std::collections::VecDeque;
use std::ffi::OsString;

pub(crate) const USAGE_ERROR_EXIT: u8 = 64;

pub(crate) enum Invocation {
    Compile(CompileArguments),
    Run(RunArguments),
    ParseSource(std::path::PathBuf),
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

#[derive(Debug)]
pub(crate) struct UsageError {
    pub message: String,
    pub exit: u8,
}

impl From<String> for UsageError {
    fn from(message: String) -> Self {
        Self {
            message,
            exit: USAGE_ERROR_EXIT,
        }
    }
}

struct Arguments {
    remaining: VecDeque<OsString>,
    verbose: bool,
}

impl Arguments {
    fn new(tokens: impl Iterator<Item = OsString>) -> Self {
        Self {
            remaining: tokens.collect(),
            verbose: false,
        }
    }

    // The command owns the next token when it asks for an option's value.
    // Only recognize global flags while looking for another argument.
    fn next_argument(&mut self) -> Option<OsString> {
        while let Some(argument) = self.next_value() {
            if argument != "--verbose" {
                return Some(argument);
            }
            self.verbose = true;
        }
        None
    }

    fn next_value(&mut self) -> Option<OsString> {
        self.remaining.pop_front()
    }
}

pub(crate) fn parse(
    tokens: impl Iterator<Item = OsString>,
) -> Result<(Invocation, DiagnosticOutput), UsageError> {
    let mut arguments = Arguments::new(tokens);
    let command = parse_command(&mut arguments)?;
    Ok((
        command,
        DiagnosticOutput {
            verbose: arguments.verbose,
        },
    ))
}

fn parse_command(arguments: &mut Arguments) -> Result<Invocation, UsageError> {
    let first = arguments.next_argument();
    match first.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("packages") => parse_packages_subsystem(arguments),
        Some("psi") => parse_psi_subsystem(arguments),
        Some("source") => return source::parse(arguments),
        Some("build") => finish_subcommand(
            compile::parse_arguments(arguments).and_then(|request| {
                if request
                    .as_ref()
                    .is_some_and(|arguments| arguments.check_only)
                {
                    return Err(
                        "use omega --check to check source without building an artifact".into(),
                    );
                }
                Ok(request)
            }),
            compile::USAGE,
            Invocation::Compile,
        ),
        Some("run") => {
            finish_subcommand(run::parse_arguments(arguments), run::USAGE, Invocation::Run)
        }
        Some(retired @ ("install" | "update" | "audit" | "inspect-terminal")) => {
            Err(retired_command(retired))
        }
        Some("--help" | "-h" | "help") => Ok(Invocation::Help(compile::USAGE)),
        Some("--version" | "-V") => {
            if arguments.next_argument().is_some() {
                return Err("omega --version takes no arguments".to_owned().into());
            }
            Ok(Invocation::Version)
        }
        _ => {
            if let Some(first) = first {
                arguments.remaining.push_front(first);
            }
            finish_subcommand(
                compile::parse_arguments(arguments),
                compile::USAGE,
                Invocation::Compile,
            )
        }
    }
    .map_err(UsageError::from)
}

fn parse_trusted_artifact(
    arguments: &mut Arguments,
    slot: &mut Option<String>,
) -> Result<(), String> {
    if slot.is_some() {
        return Err("duplicate --trusted-artifact".into());
    }
    let digest = take_text(arguments, "--trusted-artifact")?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(
            "--trusted-artifact requires the 64 hexadecimal digits issued by the trusted producer"
                .into(),
        );
    }
    *slot = Some(digest.to_ascii_lowercase());
    Ok(())
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
    arguments: &mut Arguments,
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

fn parse_packages_subsystem(arguments: &mut Arguments) -> Result<Invocation, String> {
    let Some(action) = arguments.next_argument() else {
        return Ok(Invocation::Help(packages::GUIDE));
    };
    match action.to_str() {
        Some("help") => packages::guide(arguments).map(Invocation::Help),
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
            let name = action.to_string_lossy();
            Err(format!("unknown packages command `{name}`\n{}", packages::USAGE))
        }
    }
}

fn parse_psi_subsystem(arguments: &mut Arguments) -> Result<Invocation, String> {
    let action = arguments.next_argument();
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

fn set_flag(value: &mut bool, flag: &str) -> Result<(), String> {
    if *value {
        return Err(format!("duplicate {flag}"));
    }
    *value = true;
    Ok(())
}

// Every `--target` list is one exact target request: a set the spec normalizes
// to canonical order with repeats removed
// (`wiki/spec/build/configuration.md#exact-target-requests`). `ExplicitTargetSet`
// owns that rule; no list means the command's default selection.
fn explicit_targets(names: &[String]) -> Result<Vec<target::TargetProfile>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    target::ExplicitTargetSet::from_caller_names(names)
        .map(|targets| targets.profiles().to_vec())
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })
}

fn option_value(arguments: &mut Arguments) -> Option<OsString> {
    arguments
        .next_value()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
}

fn take_value(arguments: &mut Arguments, flag: &str) -> Result<OsString, String> {
    let value = arguments
        .next_value()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if value.is_empty() || value.as_encoded_bytes().starts_with(b"-") {
        return Err(format!("{flag} requires a nonempty value, not an option"));
    }
    Ok(value)
}

fn take_text(arguments: &mut Arguments, flag: &str) -> Result<String, String> {
    take_value(arguments, flag)?
        .into_string()
        .map_err(|_| format!("{flag} requires a UTF-8 value"))
}

fn parse_directory(
    arguments: &mut Arguments,
    flag: &str,
    directories: &mut platform_custody::directory_mappings::DirectoryMappings,
) -> Result<(), String> {
    use platform_custody::directory_mappings::DirectoryAccess;
    let access = match flag {
        "--build-directories-readonly" => DirectoryAccess::ReadOnly,
        "--build-directories-readwrite" => DirectoryAccess::ReadWrite,
        _ => return Err(format!("unknown directory mapping {flag}")),
    };
    let mapping = option_value(arguments)
        .ok_or_else(|| format!("{flag} needs host-directory::/guest-directory"))?;
    let separator = mapping
        .as_encoded_bytes()
        .windows(3)
        .rposition(|part| part == b"::/")
        .filter(|index| *index > 0)
        .ok_or_else(|| format!("{flag} needs host-directory::/guest-directory"))?;
    // The prefix comes from this OsStr and ends immediately before an ASCII colon,
    // an encoding boundary on every supported host. Preserve native filename bytes.
    let prefix = unsafe {
        std::ffi::OsStr::from_encoded_bytes_unchecked(&mapping.as_encoded_bytes()[..separator])
    };
    let host = std::path::Path::new(prefix);
    let guest = std::str::from_utf8(&mapping.as_encoded_bytes()[separator + 2..])
        .map_err(|_| format!("{flag} guest directory is not UTF-8"))?;
    directories
        .grant(host, guest, access)
        .map_err(|error| format!("{flag} {}::{guest}: {error}", host.display()))
}

pub(crate) fn build_snapshot(
    capture: Option<package_compilation::BuildSourceCaptureRequest>,
    directories: platform_custody::directory_mappings::DirectoryMappings,
) -> Option<package_compilation::BuildSnapshotRequest> {
    use package_compilation::BuildSnapshotRequest;
    let snapshot = match capture {
        Some(selection) => BuildSnapshotRequest::scoped(Vec::new(), selection),
        None if directories.is_empty() => return None,
        None => BuildSnapshotRequest::for_activation(false),
    };
    Some(snapshot.with_directories(directories))
}

fn parse_build_input(
    arguments: &mut Arguments,
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
            .map(|(command, _)| command)
            .map_err(|error| error.message)
    }

    #[test]
    fn verbose_is_global_between_arguments() {
        for command in [
            &["--verbose", "--check", "main.omg"][..],
            &["run", "--engine", "psi", "--verbose", "main.omg"],
            &[
                "packages",
                "--verbose",
                "audit",
                "--project",
                ".",
                "--verbose",
            ],
            &["psi", "--verbose", "inspect", "--help"],
            &["--version", "--verbose"],
        ] {
            let (_, output) = parse(command.iter().map(OsString::from)).unwrap();
            assert!(output.verbose, "{command:?}");
        }
        let (_, output) = parse(["--version"].into_iter().map(OsString::from)).unwrap();
        assert!(!output.verbose);
    }

    #[test]
    fn option_values_see_the_token_immediately_after_the_option() {
        for (command, expected) in [
            (
                &["--report-file", "--verbose", "report.txt", "main.omg"][..],
                "--report-file requires a path",
            ),
            (
                &["run", "--engine", "--verbose", "psi", "main.omg"],
                "--engine requires",
            ),
            (
                &["packages", "audit", "--project", "--verbose", "."],
                "--project requires",
            ),
        ] {
            let error = parse_slice(command).err().expect("missing option value");
            assert!(error.starts_with(expected), "{command:?}: {error}");
        }
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
            &["packages"],
            &["packages", "help", "add"],
            &["packages", "help", "update"],
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

    // `omega` and `omega packages` read a `--target` list the same way: the
    // spec's exact target request is a set, so a repeat is dropped and the
    // profiles come back in catalog order, while `all` is still refused.
    #[test]
    fn package_commands_read_target_lists_as_one_exact_target_set() {
        use target::TargetProfile::{LinuxArm64, LinuxX64};
        let targets = [
            "--target",
            "linux_x86_64",
            "--target",
            "linux_arm64",
            "--target",
            "linux_x86_64",
        ];
        let typed = |prefix: &[&'static str]| [prefix, &targets[..]].concat();
        for command in [&["packages", "install", "lib"][..], &["packages", "update"]] {
            match parse_slice(&typed(command)) {
                Ok(Invocation::Package { options, .. }) => {
                    assert_eq!(options.targets, [LinuxArm64, LinuxX64], "{command:?}");
                }
                _ => panic!("{command:?} with a repeated --target did not parse"),
            }
        }
        match parse_slice(&typed(&["packages", "audit"])) {
            Ok(Invocation::AuditPackages(options)) => {
                assert_eq!(options.targets, [LinuxArm64, LinuxX64]);
            }
            _ => panic!("audit with a repeated --target did not parse"),
        }
        match parse_slice(&typed(&["main.omg"])) {
            Ok(Invocation::Compile(arguments)) => {
                assert_eq!(arguments.target_names, ["linux_x86_64", "linux_arm64"]);
            }
            _ => panic!("compile with a repeated --target did not parse"),
        }

        for command in [
            &["packages", "install", "lib", "--target", "all"][..],
            &["packages", "update", "--target", "all"],
            &["packages", "audit", "--target", "all"],
        ] {
            let Err(error) = parse_slice(command) else {
                panic!("{command:?} accepted the `all` wildcard");
            };
            assert!(error.contains("not an exact target profile"), "{error}");
        }
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
