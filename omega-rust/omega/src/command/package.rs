use std::ffi::OsString;
use std::path::PathBuf;

use package_manager::operations::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
    execute_package_command,
};
use target::TargetProfile;

#[cfg(test)]
mod tests;

pub(super) fn run(kind: PackageCommandKind, arguments: impl Iterator<Item = OsString>) {
    let usage = usage(&kind);
    let parsed = match parse_arguments(kind, arguments) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}\n{usage}");
            std::process::exit(2);
        }
    };
    let Some((command, options)) = parsed else {
        println!("{usage}");
        return;
    };
    let outcome = execute_package_command(command, options).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    if !outcome.report.is_empty() {
        print!("{}", outcome.report);
        if !outcome.report.ends_with('\n') {
            println!();
        }
    }
    for path in outcome.review_paths {
        println!("review: {}", path.display());
    }
    let status = exit_status(outcome.status);
    if status != 0 {
        std::process::exit(status);
    }
}

fn exit_status(status: PackageCommandStatus) -> i32 {
    match status {
        PackageCommandStatus::Published | PackageCommandStatus::ReviewDiscarded => 0,
        PackageCommandStatus::ReviewRequired => 3,
    }
}

fn usage(kind: &PackageCommandKind) -> &'static str {
    match kind {
        PackageCommandKind::Install => {
            "usage: omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline]\n       omega install --resume [--project <dir>] [--offline]\n       omega install --discard-review [--project <dir>] [--offline]\n       omega install --help\n--offline disables package source network acquisition for this invocation.\n--package selects a Git workspace member by its declared name.\n--discard-review abandons pending review; it does not discard publication recovery."
        }
        PackageCommandKind::Update => {
            "usage: omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline]\n       omega update --resume [--project <dir>] [--offline]\n       omega update --discard-review [--project <dir>] [--offline]\n       omega update --help\n--offline disables package source network acquisition for this invocation.\n--discard-review abandons pending review; it does not discard publication recovery."
        }
    }
}

fn parse_arguments(
    kind: PackageCommandKind,
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<(PackageCommand, PackageCommandOptions)>, String> {
    let mut positionals = Vec::new();
    let mut revision = None;
    let mut alias = None;
    let mut package = None;
    let mut project_root = None;
    let mut targets = Vec::new();
    let mut resume = false;
    let mut discard_review = false;
    let mut help = false;
    let mut offline = false;
    while let Some(argument) = arguments.next() {
        let argument = argument
            .into_string()
            .map_err(|_| "package arguments must be UTF-8 (except --project paths)".to_owned())?;
        match argument.as_str() {
            "--help" => set_flag(&mut help, &argument)?,
            "--offline" => set_flag(&mut offline, &argument)?,
            "--resume" => set_flag(&mut resume, &argument)?,
            "--discard-review" => set_flag(&mut discard_review, &argument)?,
            "--project" => {
                if project_root.is_some() {
                    return Err("duplicate --project".to_owned());
                }
                project_root = Some(PathBuf::from(take_value(&mut arguments, &argument)?));
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
                revision = Some(take_text(&mut arguments, &argument)?);
            }
            "--package" => {
                if !matches!(kind, PackageCommandKind::Install) {
                    return Err("--package is only valid for install".to_owned());
                }
                if package.is_some() {
                    return Err("duplicate --package".to_owned());
                }
                package = Some(take_text(&mut arguments, &argument)?);
            }
            "--as" => {
                if !matches!(kind, PackageCommandKind::Install) {
                    return Err("--as is only valid for install".to_owned());
                }
                if alias.is_some() {
                    return Err("duplicate --as".to_owned());
                }
                alias = Some(take_text(&mut arguments, &argument)?);
            }
            "--target" => {
                let name = take_text(&mut arguments, &argument)?;
                let target = TargetProfile::from_omega_target_name(Some(&name))
                    .map_err(|error| error.to_string())?;
                if targets.contains(&target) {
                    return Err(format!("duplicate target `{name}`"));
                }
                targets.push(target);
            }
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
    if resume && discard_review {
        return Err("--resume and --discard-review cannot be combined".to_owned());
    }
    if (resume || discard_review)
        && (!positionals.is_empty()
            || revision.is_some()
            || alias.is_some()
            || package.is_some()
            || !targets.is_empty())
    {
        return Err("--resume and --discard-review allow only --project and --offline".to_owned());
    }
    if help {
        return Ok(None);
    }
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
            project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
            targets,
            offline,
        },
    )))
}

fn set_flag(value: &mut bool, flag: &str) -> Result<(), String> {
    if *value {
        return Err(format!("duplicate {flag}"));
    }
    *value = true;
    Ok(())
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
