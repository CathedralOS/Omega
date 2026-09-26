use package_compilation::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};
use package_manager::operations::PackageInspectionOptions;
use std::ffi::OsString;
use std::path::PathBuf;
use target::TargetProfile;

pub(super) const USAGE: &str = "usage: omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

pub(super) fn parse(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<PackageInspectionOptions>, String> {
    let mut project_root = None;
    let mut targets = Vec::new();
    let mut help = false;
    let mut details = false;
    let mut offline = false;
    let mut build_inputs = Vec::new();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help") if !help => help = true,
            Some("--details") if !details => details = true,
            Some("--offline") if !offline => offline = true,
            Some(flag @ ("--build-input" | "--optional-build-input")) => {
                let path = super::option_value(&mut arguments)
                    .ok_or_else(|| format!("{flag} requires a canonical relative path"))?
                    .into_string()
                    .map_err(|_| format!("{flag} requires a UTF-8 canonical relative path"))?;
                let obligation = if flag == "--build-input" {
                    BuildSourceCaptureObligation::Required
                } else {
                    BuildSourceCaptureObligation::Optional
                };
                build_inputs.push((path.into_bytes(), obligation));
            }
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
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(BuildSourceCaptureRequest::new(build_inputs)?)
    };
    Ok((!help).then(|| PackageInspectionOptions {
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        targets,
        details,
        offline,
        build_inputs,
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
    use super::{OsString, PathBuf, parse};

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
        assert!(options.build_inputs.is_none());
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
            vec!["--build-input"],
            vec!["--optional-build-input", "--offline"],
            vec!["--build-input", ""],
            vec!["--build-input", "../secret"],
            vec!["--build-input", "/absolute"],
            vec!["--build-input", "C:/drive"],
            vec![
                "--build-input",
                "templates",
                "--build-input",
                "templates/file",
            ],
            vec![
                "--build-input",
                "main.omg",
                "--optional-build-input",
                "main.omg",
            ],
        ] {
            assert!(parse(arguments(&values)).is_err(), "{values:?}");
        }
    }

    #[test]
    fn inspection_retains_required_and_optional_input_obligations() {
        use package_compilation::BuildSourceCaptureObligation::{Optional, Required};
        let options = parse(arguments(&[
            "--build-input",
            "main.omg",
            "--optional-build-input",
            "settings.cfg",
            "--build-input",
            "templates",
            "--build-input",
            "-template",
            "--details",
            "--offline",
        ]))
        .unwrap()
        .unwrap();
        let inputs = options.build_inputs.unwrap();
        assert_eq!(
            inputs.entries().collect::<Vec<_>>(),
            vec![
                (b"-template".as_slice(), Required),
                (b"main.omg".as_slice(), Required),
                (b"settings.cfg".as_slice(), Optional),
                (b"templates".as_slice(), Required),
            ]
        );
        assert!(options.details && options.offline);
    }

    #[test]
    #[cfg(unix)]
    fn inspection_rejects_non_utf8_logical_input_paths() {
        use std::os::unix::ffi::OsStringExt;
        let error = parse(
            [
                OsString::from("--build-input"),
                OsString::from_vec(vec![0xff]),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(error.contains("UTF-8"), "{error}");
    }
}
