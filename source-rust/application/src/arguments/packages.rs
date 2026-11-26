// Parse arguments for `omega packages install` and `omega packages update`, including `--resume`, `--discard-review`, and `--dangerously-accept-all`.

use super::{explicit_targets, parse_build_input, take_text, take_value};
use package_compilation::BuildSourceCaptureRequest;
use package_manager::{PackageCommand, PackageCommandKind, PackageCommandOptions};
use std::path::PathBuf;

pub(super) const USAGE: &str = "usage: omega packages help <add|update>\n       omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] ...\n       omega packages update [package-or-alias...] [--to <revision>] ...\n       omega packages audit --kind <local|git> <locator> [--rev <rev>]\n       omega packages audit [--project <dir>] [--target <name>]... [--details] ...\n       omega packages <install|update> --resume [--project <dir>] [--offline]\n       omega packages <install|update> --discard-review [--project <dir>] [--offline]\nRun `omega packages <install|update|audit> --help` for that command's options.";

pub(super) const GUIDE: &str = "Declare each direct dependency in the Build that consumes it.
Run `omega packages help add` to add a dependency in source.
Run `omega packages help update` to check a candidate and edit its revision.
Run `omega packages --help` for audit and lock-based commands.";

pub(super) fn guide(arguments: &mut super::Arguments) -> Result<&'static str, String> {
    let Some(topic) = arguments.next_argument() else {
        return Ok(GUIDE);
    };
    if arguments.next_argument().is_some() {
        return Err("usage: omega packages help <add|update>".into());
    }
    match topic.to_str() {
        Some("add") => Ok(r#"Choose the Build that supplies the dependency to its immediate product.
For a helper used by a successor Build, register it in the plan returning InterpretedBuild. For application code, register it in the plan returning Application.

use {omega}/build/dependencies/providers/git.omg as git;

machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "board" };
    let layout: DependencyDescription = |git|::dependency(
        "layout", "https://example.org/layout.git", "refs/heads/stable",
        "8e76d2a18f2ecb674b75670ab380aa1cf32f9d44");
    _ = Build::dependency(plan, layout);
}

Replace the example repository and commit with the verified upstream revision. Import the library in that product with `use {layout}/api.omg;`. This example uses a service-free library."#),
        Some("update") => Ok("Find the executed Git declaration in the consuming Build. Resolve its tracking reference with Git and verify the candidate commit. Replace the revision argument in that call, keeping the alias and reach ceiling intact.
If several calls or branches match, show their filenames, locations and proposed edits so the author can choose the occurrence.
Run `omega build build.omg` on the candidate. Use `omega packages audit` to inspect the selected project and target. A historical comparison requires the old inputs or a comparable recorded result."),
        _ => Err("usage: omega packages help <add|update>".into()),
    }
}

pub(super) fn usage(kind: &PackageCommandKind) -> &'static str {
    match kind {
        PackageCommandKind::Install => {
            "usage: omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]\n       omega packages install --resume [--project <dir>] [--offline]\n       omega packages install --discard-review [--project <dir>] [--offline]\n       omega packages install --help\n--allow ProgramLaunch --program <identity> <executable> binds a program for this invocation, including --resume. Its optional --program-read, --program-write and --program-timeout limits follow the binding.\n--offline disables package source network acquisition for this invocation.\n--package selects a Git workspace member by its declared name.\n--discard-review abandons pending review; it does not discard publication recovery.\n--dangerously-accept-all accepts every pending review decision unread and publishes; omega.lock records each acceptance as if edited by hand."
        }
        PackageCommandKind::Update => {
            "usage: omega packages update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]\n       omega packages update --resume [--project <dir>] [--offline]\n       omega packages update --discard-review [--project <dir>] [--offline]\n       omega packages update --help\n--allow ProgramLaunch --program <identity> <executable> binds a program for this invocation, including --resume. Its optional --program-read, --program-write and --program-timeout limits follow the binding.\n--offline disables package source network acquisition for this invocation.\n--discard-review abandons pending review; it does not discard publication recovery.\n--dangerously-accept-all accepts every pending review decision unread and publishes; omega.lock records each acceptance as if edited by hand."
        }
    }
}

pub(super) fn parse_arguments(
    kind: PackageCommandKind,
    arguments: &mut super::Arguments,
) -> Result<Option<(PackageCommand, PackageCommandOptions, bool)>, String> {
    let mut positionals = Vec::new();
    let mut revision = None;
    let mut alias = None;
    let mut package = None;
    let mut project_root = None;
    let mut target_names = Vec::new();
    let mut resume = false;
    let mut discard_review = false;
    let mut accept_all = false;
    let mut help = false;
    let mut offline = false;
    let mut build_inputs = Vec::new();
    let mut programs = package_compilation::build_execution::BuildProgramInvocation::default();
    while let Some(argument) = arguments.next_argument() {
        if super::programs::take(&argument, arguments, &mut programs)? {
            continue;
        }
        let argument = argument
            .into_string()
            .map_err(|_| "package arguments must be UTF-8 (except --project paths)".to_owned())?;
        match argument.as_str() {
            "--help" | "-h" => super::set_flag(&mut help, &argument)?,
            "--offline" => super::set_flag(&mut offline, &argument)?,
            "--resume" => super::set_flag(&mut resume, &argument)?,
            "--discard-review" => super::set_flag(&mut discard_review, &argument)?,
            "--dangerously-accept-all" => super::set_flag(&mut accept_all, &argument)?,
            "--build-input" | "--optional-build-input" => {
                build_inputs.push(parse_build_input(arguments, &argument)?);
            }
            "--project" => {
                if project_root.is_some() {
                    return Err("duplicate --project".to_owned());
                }
                project_root = Some(PathBuf::from(take_value(arguments, &argument)?));
            }
            "--rev" | "--to" => {
                if !matches!(
                    (&kind, argument.as_str()),
                    (PackageCommandKind::Install, "--rev") | (PackageCommandKind::Update, "--to")
                ) {
                    return Err(format!("{argument} is not valid for this package command"));
                }
                if revision.is_some() {
                    return Err(format!("duplicate {argument}"));
                }
                revision = Some(take_text(arguments, &argument)?);
            }
            "--package" => {
                if !matches!(kind, PackageCommandKind::Install) {
                    return Err("--package is only valid for install".to_owned());
                }
                if package.is_some() {
                    return Err("duplicate --package".to_owned());
                }
                package = Some(take_text(arguments, &argument)?);
            }
            "--as" => {
                if !matches!(kind, PackageCommandKind::Install) {
                    return Err("--as is only valid for install".to_owned());
                }
                if alias.is_some() {
                    return Err("duplicate --as".to_owned());
                }
                alias = Some(take_text(arguments, &argument)?);
            }
            "--target" => target_names.push(take_text(arguments, &argument)?),
            _ if argument.starts_with('-') => {
                return Err(format!("unrecognized option `{argument}`"));
            }
            _ => {
                if argument.is_empty() {
                    return Err("source or package selection must not be empty".to_owned());
                }
                if matches!(kind, PackageCommandKind::Install) && !positionals.is_empty() {
                    return Err("install requires exactly one source".to_owned());
                }
                if positionals.contains(&argument) {
                    return Err(format!("duplicate package selection `{argument}`"));
                }
                positionals.push(argument);
            }
        }
    }
    super::programs::finish(&mut programs)?;
    if discard_review && programs.allow {
        return Err("--discard-review cannot accept Build grants".into());
    }
    if resume && discard_review {
        return Err("--resume and --discard-review cannot be combined".to_owned());
    }
    if discard_review && accept_all {
        return Err("--discard-review and --dangerously-accept-all cannot be combined".to_owned());
    }
    if (resume || discard_review)
        && (!positionals.is_empty()
            || revision.is_some()
            || alias.is_some()
            || package.is_some()
            || !build_inputs.is_empty()
            || !target_names.is_empty())
    {
        return Err("--resume and --discard-review allow only --project and --offline".to_owned());
    }
    if help {
        return Ok(None);
    }
    let targets = explicit_targets(&target_names)?;
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(BuildSourceCaptureRequest::new(build_inputs)?)
    };
    let command = if resume {
        PackageCommand::Resume { kind }
    } else if discard_review {
        PackageCommand::DiscardReview
    } else {
        match kind {
            PackageCommandKind::Install => PackageCommand::Install {
                source: positionals
                    .pop()
                    .ok_or_else(|| "install requires a source".to_owned())?,
                revision,
                alias,
                package,
            },
            PackageCommandKind::Update => PackageCommand::Update {
                packages: positionals,
                revision,
            },
        }
    };
    Ok(Some((
        command,
        PackageCommandOptions {
            programs,
            project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
            targets,
            offline,
            build_inputs,
        },
        accept_all,
    )))
}
