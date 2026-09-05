use package_manager::operations::{PackageInspectionOptions, inspect_packages};
use std::ffi::OsString;
use std::path::PathBuf;
use target::TargetProfile;

pub(super) const USAGE: &str = "usage: omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline]\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

pub(super) fn run(arguments: impl Iterator<Item = OsString>) {
    let options = match parse(arguments) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return;
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(2);
        }
    };
    match inspect_packages(options) {
        Ok(outcome) => {
            print!("{}", outcome.report);
            if !outcome.complete {
                std::process::exit(1);
            }
            if outcome.requires_decision {
                std::process::exit(3);
            }
        }
        Err(error) => {
            eprintln!("cannot inspect packages: {error}");
            std::process::exit(1);
        }
    }
}

fn parse(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<PackageInspectionOptions>, String> {
    let mut project_root = None;
    let mut targets = Vec::new();
    let mut help = false;
    let mut details = false;
    let mut offline = false;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help") if !help => help = true,
            Some("--details") if !details => details = true,
            Some("--offline") if !offline => offline = true,
            Some("--project") if project_root.is_none() => {
                project_root = Some(PathBuf::from(value(&mut arguments, "--project")?));
            }
            Some("--target") => {
                let name = value(&mut arguments, "--target")?
                    .into_string()
                    .map_err(|_| "--target requires UTF-8".to_owned())?;
                let target = TargetProfile::from_omega_target_name(Some(&name))
                    .map_err(|error| error.to_string())?;
                if targets.contains(&target) {
                    return Err(format!("duplicate target {name:?}"));
                }
                targets.push(target);
            }
            _ => {
                return Err(format!(
                    "unexpected or duplicate inspection argument {argument:?}"
                ));
            }
        }
    }
    Ok((!help).then(|| PackageInspectionOptions {
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        targets,
        details,
        offline,
    }))
}

fn value(arguments: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString, String> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"-"))
        .ok_or_else(|| format!("{flag} requires a nonempty value, not an option"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> impl Iterator<Item = OsString> {
        values.iter().map(OsString::from)
    }

    #[test]
    fn target_selection_and_default_project_are_explicit() {
        let options = parse(arguments(&[])).unwrap().unwrap();
        assert_eq!(options.project_root, PathBuf::from("."));
        assert!(options.targets.is_empty());
        assert!(!options.details);
        assert!(!options.offline);
        assert!(parse(arguments(&["--details"])).unwrap().unwrap().details);
        let options = parse(arguments(&[
            "--project",
            "space in path",
            "--offline",
            "--target",
            "linux_x86_64",
            "--target",
            "macos_arm64",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(options.project_root, PathBuf::from("space in path"));
        assert_eq!(options.targets.len(), 2);
        assert!(options.offline);
        assert!(parse(arguments(&["--help"])).unwrap().is_none());
    }

    #[test]
    fn malformed_options_are_rejected() {
        for values in [
            vec!["extra"],
            vec!["--rev", "main"],
            vec!["--target"],
            vec!["--project", "--target"],
            vec!["--project", ""],
            vec!["--target", "not-a-target"],
            vec!["--help", "--help"],
            vec!["--details", "--details"],
            vec!["--offline", "--offline"],
            vec!["--project", "--offline"],
            vec!["--target", "--offline"],
            vec!["--project", ".", "--project", "."],
            vec!["--target", "linux_x86_64", "--target", "linux_x86_64"],
        ] {
            assert!(parse(arguments(&values)).is_err(), "{values:?}");
        }
    }
}
