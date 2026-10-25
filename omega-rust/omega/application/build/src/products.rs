//! Translate project product demands into package or standalone facade requests.

use crate::{BuildError, BuildRequest, BuildTargets, ProjectProduct};
use compiler::{CompilationOutput, CompileRequest, RequestedCompileProduct};
use package_manager::operations as packages;
use std::path::Path;
use target::TargetProfile;

pub(crate) fn compile(
    request: &BuildRequest,
    prepared: Option<packages::PreparedLocalProject>,
    directory: &Path,
    targets: &[TargetProfile],
) -> Result<Vec<(TargetProfile, Result<CompilationOutput, BuildError>)>, BuildError> {
    let mut reports = Vec::with_capacity(targets.len());
    match (prepared, &request.product) {
        (
            Some(prepared),
            ProjectProduct::Native {
                private_entry_ram, ..
            },
        ) => {
            let mut product = packages::PreparedLocalProjectNativeRequest::for_targets(
                prepared,
                directory,
                targets.to_vec(),
            )
            .with_optimization_rollback(request.optimization_rollback.clone())
            .with_private_entry_ram(private_entry_ram.clone())
            .with_timings(request.timings);
            if let Some(snapshot) = &request.build_snapshot {
                product = product.with_build_snapshot(snapshot.clone());
            }
            for outcome in packages::compile_prepared_local_project_for_native_targets(product) {
                reports.push((
                    outcome.target,
                    outcome
                        .result
                        .map(CompilationOutput::Report)
                        .map_err(BuildError::PackageNative),
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
                    outcome
                        .result
                        .map(CompilationOutput::Report)
                        .map_err(BuildError::PackageCheck),
                ));
            }
        }
        (prepared, ProjectProduct::Inspection { machine }) => {
            let [profile] = targets else {
                return Err(BuildError::Diagnostics(vec![
                    diagnostics::Diagnostic::error(
                        "Terminal inspection lost its single target context",
                    ),
                ]));
            };
            let output = match prepared {
                Some(prepared) => {
                    packages::inspect_prepared_local_project(prepared, directory, *profile, machine)
                        .map(CompilationOutput::Inspection)
                        .map_err(BuildError::Inspection)
                }
                None => {
                    let target = (!matches!(request.targets, BuildTargets::Host))
                        .then_some(profile.target_name());
                    let mut source =
                        compiler::CheckedCompileRequest::new(&request.root_path, target);
                    source.build_dir = Some(directory.to_path_buf());
                    compiler::CompilationSession::default()
                        .produce(
                            source,
                            compiler::CompilationProduct::Inspection(machine.clone()),
                        )
                        .map_err(|error| BuildError::Diagnostics(error.into_diagnostics()))
                }
            };
            reports.push((*profile, output));
        }
        (None, ProjectProduct::Check) => {
            return compile_standalone_reports(
                request,
                directory,
                targets,
                RequestedCompileProduct::Check,
                &[],
            );
        }
        (
            None,
            ProjectProduct::Native {
                private_entry_ram, ..
            },
        ) => {
            return compile_standalone_reports(
                request,
                directory,
                targets,
                RequestedCompileProduct::NativeArtifact,
                private_entry_ram,
            );
        }
    }
    Ok(reports)
}

/// Standalone check/native reports use the batch compiler facade. Inspection
/// has a different terminal product, selected above rather than flattened here.
fn compile_standalone_reports(
    request: &BuildRequest,
    directory: &Path,
    targets: &[TargetProfile],
    product: RequestedCompileProduct,
    private_entry_ram: &[compilation_products::PrivateEntryRamInitialization],
) -> Result<Vec<(TargetProfile, Result<CompilationOutput, BuildError>)>, BuildError> {
    let mut reports = Vec::with_capacity(targets.len());
    let explicit = !matches!(request.targets, BuildTargets::Host);
    let profiles = targets
        .iter()
        .map(|profile| explicit.then_some(*profile))
        .collect();
    let mut compilation =
        CompileRequest::for_targets(request.root_path.clone(), directory.to_path_buf(), profiles)
            .with_requested_product(product)
            .with_optimization_rollback(request.optimization_rollback.clone())
            .with_timings(request.timings);
    compilation = compilation.with_private_entry_ram(private_entry_ram.to_vec());
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
            outcome
                .into_result()
                .map(CompilationOutput::Report)
                .map_err(BuildError::Diagnostics),
        ));
    }
    Ok(reports)
}
