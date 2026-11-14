// Parse arguments for `omega packages audit`.
//
// `--kind` selects the package-source locator form. Without it, audit inspects
// the named project or the current directory. The two forms do not combine:
// `--rev` belongs to the locator, and project flags stay on the project form.

use super::{parse_build_input, set_flag, take_text, take_value};
use package_compilation::BuildSourceCaptureRequest;
use package_manager::operations::PackageInspectionOptions;
use std::ffi::OsString;
use std::path::PathBuf;
use target::TargetProfile;

pub(super) const USAGE: &str = "usage: omega packages audit --kind <local|git> <locator> [--rev <rev>]\n       omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...\nChecks current project source with accepted dependency pins; no lock means fresh unaccepted inspection.\n--offline disables package source network acquisition for this invocation.\n--details includes full normalized policy after the readable summary.\nExit 0: checked; 1: unavailable; 2: invalid arguments; 3: policy requires review.\nInspection never accepts changes or resumes a pending publication.";

pub(super) enum ParsedAudit {
    Locator(SourceArguments),
    Project(PackageInspectionOptions),
}

pub(crate) struct SourceArguments {
    pub(crate) source_kind: String,
    pub(crate) locator: String,
    pub(crate) rev: Option<String>,
}

pub(super) fn parse(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<ParsedAudit>, String> {
    let mut project_root = None;
    let mut targets = Vec::new();
    let mut help = false;
    let mut details = false;
    let mut offline = false;
    let mut build_inputs = Vec::new();
    let mut source_kind = None;
    let mut rev = None;
    let mut locator = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help" | "-h") => set_flag(&mut help, &argument.to_string_lossy())?,
            Some("--details") => set_flag(&mut details, "--details")?,
            Some("--offline") => set_flag(&mut offline, "--offline")?,
            Some(flag @ ("--build-input" | "--optional-build-input")) => {
                build_inputs.push(parse_build_input(&mut arguments, flag)?);
            }
            Some("--kind") => {
                if source_kind.is_some() {
                    return Err("duplicate --kind".into());
                }
                let kind = take_text(&mut arguments, "--kind")?;
                if kind != "local" && kind != "git" {
                    return Err(format!("--kind requires `local` or `git`, not `{kind}`"));
                }
                source_kind = Some(kind);
            }
            Some("--rev") => {
                if rev.is_some() {
                    return Err("duplicate --rev".into());
                }
                rev = Some(take_text(&mut arguments, "--rev")?);
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
    let project_arguments = project_root.is_some()
        || !targets.is_empty()
        || details
        || offline
        || !build_inputs.is_empty();
    let locator_arguments = source_kind.is_some() || rev.is_some() || locator.is_some();
    if locator_arguments && project_arguments {
        return Err(
            "a --kind package-source audit cannot include project audit arguments".to_owned(),
        );
    }
    if locator_arguments {
        return Ok(Some(ParsedAudit::Locator(SourceArguments {
            source_kind: source_kind.ok_or_else(|| {
                "--kind <local|git> is required to audit a package source".to_owned()
            })?,
            locator: locator.ok_or_else(|| "missing source locator".to_owned())?,
            rev,
        })));
    }
    let build_inputs = if build_inputs.is_empty() {
        None
    } else {
        Some(BuildSourceCaptureRequest::new(build_inputs)?)
    };
    Ok(Some(ParsedAudit::Project(PackageInspectionOptions {
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        targets,
        details,
        offline,
        build_inputs,
    })))
}
