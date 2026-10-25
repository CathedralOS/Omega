//! Translate project product demands into package or standalone facade requests.

use crate::{BuildError, BuildRequest, BuildTargets, ProjectProduct};
use compiler::{CompileReport, CompileRequest, RequestedCompileProduct};
use package_manager::operations as packages;
use std::path::Path;
use target::TargetProfile;

pub(crate) fn compile(
    request: &BuildRequest,
    prepared: Option<packages::PreparedLocalProject>,
    directory: &Path,
    targets: &[TargetProfile],
) -> Result<Vec<(TargetProfile, Result<CompileReport, BuildError>)>, BuildError> {
    let mut reports = Vec::with_capacity(targets.len());
    match (prepared, request.product) {
        (Some(prepared), ProjectProduct::NativeArtifact) => {
            let mut product = packages::PreparedLocalProjectNativeRequest::for_targets(
                prepared,
                directory,
                targets.to_vec(),
            )
            .with_optimization_rollback(request.optimization_rollback.clone())
            .with_timings(request.timings);
            if let Some(snapshot) = &request.build_snapshot {
                product = product.with_build_snapshot(snapshot.clone());
            }
            for outcome in packages::compile_prepared_local_project_for_native_targets(product) {
                reports.push((
                    outcome.target,
                    outcome.result.map_err(BuildError::PackageNative),
                ));
            }
        }
        (Some(prepared), ProjectProduct::Check) => {
            RequestedCompileProduct::Check
                .validate_optimization_rollback(&request.optimization_rollback)
                .map_err(|error| BuildError::Diagnostics(vec![error]))?;
            let mut product = packages::PreparedLocalProjectCheckRequest::for_targets(
                prepared,
                directory,
                targets.to_vec(),
            )
            .with_verify(request.verify)
            .with_timings(request.timings);
            if let Some(snapshot) = &request.build_snapshot {
                product = product.with_build_snapshot(snapshot.clone());
            }
            for outcome in packages::check_prepared_local_project_for_targets(product) {
                reports.push((
                    outcome.target,
                    outcome.result.map_err(BuildError::PackageCheck),
                ));
            }
        }
        (None, product) => {
            let product = match product {
                ProjectProduct::Check => RequestedCompileProduct::Check,
                ProjectProduct::NativeArtifact => RequestedCompileProduct::NativeArtifact,
            };
            let explicit = !matches!(request.targets, BuildTargets::Host);
            let profiles = targets
                .iter()
                .map(|profile| explicit.then_some(*profile))
                .collect();
            let mut compilation = CompileRequest::for_targets(
                request.root_path.clone(),
                directory.to_path_buf(),
                profiles,
            )
            .with_requested_product(product)
            .with_optimization_rollback(request.optimization_rollback.clone())
            .with_timings(request.timings);
            if let Some(snapshot) = &request.build_snapshot {
                compilation = compilation.with_build_snapshot(snapshot.clone());
            }
            let outcomes = compiler::compile(compilation).map_err(BuildError::Diagnostics)?;
            for outcome in outcomes.into_outcomes().into_vec() {
                let profile = match (outcome.target_profile(), &request.targets, targets) {
                    (Some(profile), _, _) => profile,
                    (None, BuildTargets::Host, [profile]) => *profile,
                    _ => {
                        return Err(BuildError::Diagnostics(vec![
                            diagnostics::Diagnostic::error(
                                "standalone compilation lost its requested target identity",
                            ),
                        ]));
                    }
                };
                reports.push((
                    profile,
                    outcome.into_result().map_err(BuildError::Diagnostics),
                ));
            }
        }
    }
    Ok(reports)
}
