//! Translate project product demands into package or standalone facade requests.

use crate::{BuildError, BuildRequest, BuildTargets, ProjectProduct};
use compiler::{CompilationOutput, CompileRequest, RequestedCompileProduct};
use std::path::Path;
use target::TargetProfile;

pub(crate) fn compile(
    request: &BuildRequest,
    prepared: Option<(crate::ResolvedProjectBuilds, crate::BuildSession)>,
    directory: &Path,
    targets: &[TargetProfile],
) -> Result<Vec<(TargetProfile, Result<CompilationOutput, BuildError>)>, BuildError> {
    let mut reports = Vec::with_capacity(targets.len());
    if let Some((resolved, session)) = prepared {
        let product = match &request.product {
            ProjectProduct::Check => compiler::CompilationProduct::Check,
            ProjectProduct::Terminal => {
                compiler::CompilationProduct::Terminal(request.optimization_rollback.clone())
            }
            ProjectProduct::Inspection { machine } => {
                compiler::CompilationProduct::Inspection(machine.clone())
            }
            ProjectProduct::Native {
                destination,
                private_entry_ram,
                installation_assumptions,
            } => compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
                // The complete request was validated before discovery. Targets
                // refused by output admission keep their refusal and no product.
                installation_assumptions: installation_assumptions
                    .iter()
                    .filter(|row| targets.contains(&row.profile()))
                    .cloned()
                    .collect(),
                retain_native_inputs: false,
                image: destination.image_demand(),
                private_entry_ram: private_entry_ram.clone(),
                terminal_authority_policy:
                    compilation_products::compile_request::current_terminal_authority_policy(),
                terminal_authority_permission_policy: None,
                optimization_rollback: request.optimization_rollback.clone(),
            }),
        };
        let outcomes = crate::project::project_compilation::compile_returned(
            crate::project::project_compilation::ProjectCompilationRequest {
                prepared: resolved.prepared,
                target_profiles: targets,
                snapshot: request.build_snapshot.as_ref(),
                verify: request.verify,
                timings: request.timings,
                product,
            },
            resolved.builds,
            session,
            resolved.execution_profile,
        );
        return Ok(outcomes
            .into_iter()
            .map(|outcome| {
                (
                    outcome.target,
                    outcome.result.map_err(BuildError::ProjectCompilation),
                )
            })
            .collect());
    }
    match &request.product {
        ProjectProduct::Inspection { machine } => {
            let [profile] = targets else {
                return Err(BuildError::Diagnostics(vec![
                    diagnostics::Diagnostic::error(
                        "Terminal inspection lost its single target context",
                    ),
                ]));
            };
            let output = {
                let target = (!matches!(request.targets, BuildTargets::Host))
                    .then_some(profile.target_name());
                let mut source = compiler::CheckedCompileRequest::new(&request.root_path, target);
                source.build.directory = Some(directory.to_path_buf());
                crate::produce_source(
                    &mut crate::build_session::compiler_session(),
                    source,
                    compiler::CompilationProduct::Inspection(machine.clone()),
                )
                .map_err(|error| BuildError::Diagnostics(error.into_diagnostics()))
            };
            reports.push((*profile, output));
        }
        ProjectProduct::Check => {
            return compile_standalone_reports(
                request,
                directory,
                targets,
                RequestedCompileProduct::Check,
                &[],
                &[],
                Default::default(),
            );
        }
        ProjectProduct::Terminal => {
            return compile_standalone_reports(
                request,
                directory,
                targets,
                RequestedCompileProduct::TerminalArtifact,
                &[],
                &[],
                Default::default(),
            );
        }
        ProjectProduct::Native {
            destination,
            private_entry_ram,
            installation_assumptions,
        } => {
            return compile_standalone_reports(
                request,
                directory,
                targets,
                RequestedCompileProduct::NativeArtifact,
                private_entry_ram,
                installation_assumptions,
                destination.image_demand(),
            );
        }
    }
    Ok(reports)
}

/// Standalone check/Terminal/native reports use the batch compiler facade. Inspection
/// has a different terminal product, selected above rather than flattened here.
fn compile_standalone_reports(
    request: &BuildRequest,
    directory: &Path,
    targets: &[TargetProfile],
    product: RequestedCompileProduct,
    private_entry_ram: &[compilation_products::PrivateEntryRamInitialization],
    installation_assumptions: &[compilation_products::InstallationEnvironmentAssumption],
    native_image: compilation_products::NativeImageDemand,
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
    compilation = compilation
        .with_installation_assumptions(
            installation_assumptions
                .iter()
                .filter(|row| targets.contains(&row.profile()))
                .cloned()
                .collect(),
        )
        .with_private_entry_ram(private_entry_ram.to_vec())
        .with_native_image_demand(native_image);
    if let Some(snapshot) = &request.build_snapshot {
        compilation = compilation.with_build_snapshot(snapshot.clone());
    }
    let outcomes =
        crate::compile_sources(&mut crate::build_session::compiler_session(), compilation)
            .map_err(BuildError::Diagnostics)?;
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
