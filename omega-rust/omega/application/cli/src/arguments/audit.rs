use package_compilation::BuildSourceCaptureRequest;
use package_manager::operations::PackageInspectionOptions;
use std::ffi::OsString;
use std::path::PathBuf;
use target::TargetProfile;

pub(super) const PACKAGES_USAGE: &str = "usage: omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

pub(crate) struct SourceArguments {
    pub(crate) source_kind: String,
    pub(crate) locator: String,
    pub(crate) rev: Option<String>,
}

pub(super) fn parse_packages(
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
                build_inputs.push(super::parse_build_input(&mut arguments, flag)?);
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

pub(super) fn parse_source(
    mut arguments: impl Iterator<Item = OsString>,
) -> Option<SourceArguments> {
    let mut locator = None;
    let mut source_kind = None;
    let mut rev = None;
    while let Some(argument) = arguments.next() {
        if argument == "--kind" {
            if source_kind.is_some() {
                return None;
            }
            source_kind = arguments.next().and_then(|value| value.into_string().ok());
            source_kind.as_ref()?;
            continue;
        }
        if argument == "--rev" {
            if rev.is_some() {
                return None;
            }
            rev = arguments.next().and_then(|value| value.into_string().ok());
            rev.as_ref()?;
            continue;
        }
        if locator.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        locator = Some(argument.into_string().ok()?);
    }
    Some(SourceArguments {
        source_kind: source_kind?,
        locator: locator?,
        rev,
    })
}

fn value(arguments: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString, String> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"-"))
        .ok_or_else(|| format!("{flag} requires a nonempty value, not an option"))
}
