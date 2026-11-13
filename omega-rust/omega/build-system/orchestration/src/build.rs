/*

Top-level `build(request)` lifecycle for a single or multi-target project request.

Every `build` call runs on the compiler's sized worker thread
(`compiler::checked::run_on_compile_thread`) and executes five steps in order:
1. `build_directory::admit_outputs`: claims per-target or single-target output
   directories (holding the `.omega-occupant` guards for the duration of `build`)
   or allocates a disposable `check` temporary workspace when `ProjectProduct::Check`
   or `ProjectProduct::Inspection` is requested. When a caller does not pass
   `build_directory`, the default path is anchored to the authored `root_path`
   rather than any resolver snapshot.
2. `packages::begin_local_project_preparation`: discovers whether `root_path`
   belongs to a package project (`build.omg`) and performs any pending package
   transaction recovery. This runs even when `require_package_project` is set or
   every target output directory was refused, so interrupted package transactions
   still recover cleanly.
3. `resolve_project_sources`: if `root_path` is a package project and at least
   one target output directory was admitted, opens a `BuildSession` and executes
   the project's `build.omg` graph to obtain `ResolvedProjectBuilds`.
4. `products::compile`: compiles all admitted targets in one pass (either through
   `project_compilation::compile_returned` for package projects or
   `compile_sources` / `produce_source` for standalone `.omg` files).
5. `publication::publish_products`: joins each target's directory admission outcome
   with its compilation outcome and publishes native artifacts where requested,
   preserving target order even when a subset of targets failed directory admission
   or compilation.

*/

use crate::project::{build_directory, products, publication};
use crate::{BuildError, BuildRequest, BuildTargets, ProjectProduct, TargetBuildOutcome};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use platform_custody::temporary_directory::TemporaryDirectory;
use target::TargetProfile;

pub fn build(request: BuildRequest) -> Result<Vec<TargetBuildOutcome>, BuildError> {
    // Checking and lowering keep large frames, so a build runs on the compiler's
    // sized worker like every sibling entry (`compile_sources`, package builds),
    // whichever thread the caller started on.
    compiler::checked::run_on_compile_thread(|| build_on_compile_thread(request))
}

fn build_on_compile_thread(request: BuildRequest) -> Result<Vec<TargetBuildOutcome>, BuildError> {
    let profiles = target_profiles(&request.targets)?;
    validate_request(&request, &profiles)?;
    let mut timings = if request.timings {
        CompileTimings::enabled()
    } else {
        CompileTimings::default()
    };
    let (check_workspace, base_directory) = select_base_directory(&request)?;
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
                packages::begin_local_project_preparation(
                    &request.root_path,
                    &profiles,
                    request.offline,
                )
            },
        )
        .map_err(BuildError::Preparation)?;
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
        let prepared = match prepared {
            Some(preparation) => {
                let mut session = crate::BuildSession::create_retained(&base_directory)
                    .map_err(|error| BuildError::BuildDirectory(error.to_string()))?;
                let resolved = crate::resolve_project_sources(
                    &mut session,
                    crate::ProjectSourceRequest {
                        preparation,
                        execution: package_compilation::BuildExecutionInputs {
                            snapshot: request.build_snapshot.clone(),
                            ..Default::default()
                        },
                        collect_timings: request.timings,
                    },
                );
                let resolved = match resolved {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        let mut reason = format!("project source discovery failed: {error}");
                        if let Err(error) = session.close() {
                            reason.push_str(&format!("; session disposal: {error}"));
                        }
                        return Err(BuildError::Diagnostics(vec![Diagnostic::error(reason)]));
                    }
                };
                Some((resolved, session))
            }
            None => None,
        };
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

fn validate_request(request: &BuildRequest, profiles: &[TargetProfile]) -> Result<(), BuildError> {
    if let ProjectProduct::Native {
        installation_assumptions,
        ..
    } = &request.product
    {
        compilation_products::compile_request::validate_installation_assumption_targets(
            installation_assumptions,
            profiles,
        )
        .map_err(|reason| BuildError::Diagnostics(vec![Diagnostic::error(reason)]))?;
    }
    if matches!(request.product, ProjectProduct::Check) {
        compiler::RequestedCompileProduct::Check
            .validate_optimization_rollback(&request.optimization_rollback)
            .map_err(|error| BuildError::Diagnostics(vec![error]))?;
    }
    if matches!(request.product, ProjectProduct::Inspection { .. }) && profiles.len() != 1 {
        return Err(BuildError::Diagnostics(vec![Diagnostic::error(
            "Terminal inspection requires one exact target context",
        )]));
    }
    Ok(())
}

fn select_base_directory(
    request: &BuildRequest,
) -> Result<(Option<TemporaryDirectory>, std::path::PathBuf), BuildError> {
    let check_workspace = match &request.product {
        ProjectProduct::Check | ProjectProduct::Inspection { .. } => {
            Some(TemporaryDirectory::create("check", false).map_err(BuildError::CheckWorkspace)?)
        }
        ProjectProduct::Terminal | ProjectProduct::Native { .. } => None,
    };
    let base_directory = match &check_workspace {
        Some(workspace) => workspace.path().to_path_buf(),
        None => compiler::CompileOptions {
            root_path: request.root_path.clone(),
            build_dir: request.build_directory.clone(),
            target_name: None,
        }
        .build_dir(),
    };
    Ok((check_workspace, base_directory))
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
