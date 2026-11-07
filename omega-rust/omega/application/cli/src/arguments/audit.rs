use super::{parse_build_input, set_flag, take_text, take_value};
use package_compilation::BuildSourceCaptureRequest;
use package_manager::operations::PackageInspectionOptions;
use std::ffi::OsString;
use std::path::PathBuf;
use target::TargetProfile;

pub(super) const SOURCE_USAGE: &str =
    "usage: omega audit source --kind <local|git> <locator> [--rev <rev>]";

pub(super) const PACKAGES_USAGE: &str = "usage: omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

pub(super) const USAGE: &str = "usage: omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

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
            Some("--help" | "-h") => set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--details") => set_flag(&mut details, "--details")?,
            Some("--offline") => set_flag(&mut offline, "--offline")?,
            Some(flag @ ("--build-input" | "--optional-build-input")) => {
                build_inputs.push(parse_build_input(&mut arguments, flag)?);
            }
            Some("--project") => {
                if project_root.is_some() {
                    return Err("duplicate --project".to_owned());
                }
                project_root = Some(PathBuf::from(take_value(&mut arguments, "--project")?));
            }
            Some("--target") => {
                let name = take_text(&mut arguments, "--target")?;
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
    if help {
        return Ok(None);
    }
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(BuildSourceCaptureRequest::new(build_inputs)?)
    };
    Ok(Some(PackageInspectionOptions {
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        targets,
        details,
        offline,
        build_inputs,
    }))
}

pub(super) fn parse_source(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<SourceArguments>, String> {
    let mut help = false;
    let mut locator = None;
    let mut source_kind = None;
    let mut rev = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help" | "-h") => set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--kind") => {
                if source_kind.is_some() {
                    return Err("duplicate --kind".into());
                }
                source_kind = Some(take_text(&mut arguments, "--kind")?);
            }
            Some("--rev") => {
                if rev.is_some() {
                    return Err("duplicate --rev".into());
                }
                rev = Some(take_text(&mut arguments, "--rev")?);
            }
            _ => {
                if argument.as_encoded_bytes().starts_with(b"-") {
                    return Err(format!(
                        "unrecognized option `{}`",
                        argument.to_string_lossy()
                    ));
                }
                if locator.is_some() {
                    return Err(format!(
                        "unexpected extra argument `{}`",
                        argument.to_string_lossy()
                    ));
                }
                locator = Some(
                    argument
                        .into_string()
                        .map_err(|_| "source locator must be UTF-8".to_owned())?,
                );
            }
        }
    }
    if help {
        return Ok(None);
    }
    Ok(Some(SourceArguments {
        source_kind: source_kind.ok_or_else(|| "missing --kind <local|git>".to_owned())?,
        locator: locator.ok_or_else(|| "missing source locator".to_owned())?,
        rev,
    }))
}
