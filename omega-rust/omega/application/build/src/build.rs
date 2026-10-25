//! Admit output locations, prepare a project, compile products, then publish.

use crate::{BuildError, BuildRequest, BuildTargets, ProjectProduct, TargetBuildOutcome};
use crate::{build_directory, products, publication};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use platform_custody::temporary_directory::TemporaryDirectory;
use target::TargetProfile;

/// Build all requested targets through one project preparation. Refused output
/// locations and failed products retain their own ordered outcome; siblings
/// continue. A check uses a private workspace and publishes no product files.
pub fn build(request: BuildRequest) -> Result<Vec<TargetBuildOutcome>, BuildError> {
    let profiles = target_profiles(&request.targets)?;
    if matches!(request.product, ProjectProduct::Inspection { .. }) && profiles.len() != 1 {
        return Err(BuildError::Diagnostics(vec![Diagnostic::error(
            "Terminal inspection requires one exact target context",
        )]));
    }
    let mut timings = if request.timings {
        CompileTimings::enabled()
    } else {
        CompileTimings::default()
    };
    let check_workspace = match &request.product {
        ProjectProduct::Check | ProjectProduct::Inspection { .. } => {
            Some(TemporaryDirectory::create("check", false).map_err(BuildError::CheckWorkspace)?)
        }
        ProjectProduct::Native { .. } => None,
    };
    // Placement belongs to the authored source, never its resolver snapshot.
    let base_directory = match &check_workspace {
        Some(workspace) => workspace.path().to_path_buf(),
        None => compiler::CompileOptions {
            root_path: request.root_path.clone(),
            build_dir: request.build_directory.clone(),
            target_name: None,
        }
        .build_dir(),
    };
    let (work, _occupancies) = build_directory::admit_outputs(
        &profiles,
        &base_directory,
        &request.root_path,
        matches!(request.targets, BuildTargets::Batch(_)),
        check_workspace.is_some(),
    );
    let prepared = timings
        .record_result(
            StageMeta::new("prepare", "project", "prepared sources"),
            || {
                packages::prepare_local_project_for_targets(
                    &request.root_path,
                    &profiles,
                    request.offline,
                )
            },
        )
        .map_err(BuildError::Preparation)?;
    // Discovery performs pending-transaction recovery, including when the
    // caller requires a package or every output directory was refused.
    if request.require_package_project && prepared.is_none() {
        return Err(BuildError::Diagnostics(vec![Diagnostic::error(
            "compilation requires a package project with a sibling build.omg",
        )]));
    }
    let active = work
        .iter()
        .filter(|target| target.directory.is_ok())
        .map(|target| target.profile)
        .collect::<Vec<_>>();
    let reports = if active.is_empty() {
        Vec::new()
    } else {
        timings.record_result(
            StageMeta::new("compile", "sources", "requested product"),
            || products::compile(&request, prepared, &base_directory, &active),
        )?
    };

    Ok(publication::publish_products(
        work,
        reports,
        &request.product,
        &mut timings,
    ))
}

fn target_profiles(targets: &BuildTargets) -> Result<Vec<TargetProfile>, BuildError> {
    match targets {
        BuildTargets::Single(profile) => Ok(vec![*profile]),
        BuildTargets::Batch(targets) => Ok(targets.profiles().to_vec()),
        BuildTargets::Host => match TargetProfile::host_if_supported() {
            Some(profile) => Ok(vec![profile]),
            None => Err(BuildError::Diagnostics(vec![Diagnostic::error(
                "no target was named and this host has no catalogued Omega deployment \
                 profile; name an exact target (linux_arm64, linux_x86_64, macos_arm64, \
                 macos_x86_64, windows_x86_64, uefi_x86_64, cross_platform_cli, \
                 local_unchecked, or alpha_bootstrap)",
            )])),
        },
    }
}
