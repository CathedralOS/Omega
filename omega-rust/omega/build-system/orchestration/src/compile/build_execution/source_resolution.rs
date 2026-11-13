/*

Execute acquired `build.omg` sources, resolve dependency requests, and retain
returned invocations (`resolve_project_sources`, `resolve_acquired_sources`).

`package_manager` owns source acquisition and dependency-graph validation, while
this module executes the `build.omg` activations through `ReadinessScheduler`
and retains each `ResolvedPackageBuild` (its compiler invocation paired with its
resolved `PackageCompilationInputs`) through the final source join:
- `ProjectSourceRequest` supplies the inputs known before the root `build.omg`
  executes (the `LocalProjectPreparation`, root `BuildExecutionInputs`, and
  whether timings are enabled). Execution inputs apply only to the root
  activation; dependencies run under session resources.
- `ResolvedProjectBuilds` holds the discovery-ordered `Vec<ResolvedPackageBuild>`
  alongside `PreparedLocalProject` and the host `execution_profile`.
- `ResolvedPackageBuild::settle` (and batch `settle_source_builds`) joins the
  product context and settles the original `build.omg` execution into a
  `CompletedBuild` without re-running `build.omg` or typing a `Product` yet. If
  a helper dependency's `build.omg` was already settled early during a provider
  readiness pause (`ResolvedBuild::Settled`), `settle` verifies that its source
  inputs match the final resolved graph.
- `ResolvedPackageBuild::complete` settles the build and immediately prepares
  the target `PackageBuild` product.

*/

mod readiness;

use crate::BuildSession;
use crate::compile::build_execution::ReturnedPackageBuild;
use compiler::{CompilationSession, CompileFailure, CompletedBuild, PackageBuild};
use package_compilation::PackageCompilationInputs;
use package_manager::operations::{
    LocalProjectPreparation, PrepareLocalProjectError, PreparedLocalProject,
};
use package_manager::resolution::source::{
    AcquiredBuildSource, BuildSourceAcquisitions, ResolvedBuildSources,
};
use target::TargetProfile;

#[derive(Debug)]
pub enum ProjectSourceResolutionError {
    Preparation(PrepareLocalProjectError),
    Compilation(CompileFailure),
    Source(String),
    Resources(package_manager::review::CompileResolvedPackageReviewsError),
}

impl std::fmt::Display for ProjectSourceResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparation(error) => error.fmt(formatter),
            Self::Compilation(CompileFailure::Diagnostics(diagnostics)) => {
                diagnostics::write_bounded_diagnostics(formatter, diagnostics)
            }
            Self::Compilation(CompileFailure::BuildConsentRequired(requests)) => write!(
                formatter,
                "restricted Build execution requires explicit consent before effects; pending requests: {requests:?}"
            ),
            Self::Source(reason) => formatter.write_str(reason),
            Self::Resources(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProjectSourceResolutionError {}

pub struct ProjectSourceRequest {
    pub preparation: LocalProjectPreparation,
    pub execution: package_compilation::BuildExecutionInputs,
    pub collect_timings: bool,
}

pub struct ResolvedProjectBuilds {
    pub prepared: PreparedLocalProject,
    pub builds: Vec<ResolvedPackageBuild>, // Discovery-ordered; consumers match by package identity, not path.
    pub execution_profile: TargetProfile,
}

pub struct ResolvedPackageBuild {
    build: ResolvedBuild,
    inputs: PackageCompilationInputs,
}

enum ResolvedBuild {
    Returned(ReturnedPackageBuild),
    Settled(CompletedBuild),
}

struct CatalogMemberBuild {
    capture: package_compilation::CapturedLocalSource,
    build: ReturnedPackageBuild,
    observation: package_manager::resolution::source::WorkspaceMemberObservation,
}

impl ResolvedPackageBuild {
    pub fn package_inputs(&self) -> &PackageCompilationInputs {
        &self.inputs
    }

    pub fn settle(
        self,
        compiler: &mut CompilationSession,
        target: TargetProfile,
        inputs: PackageCompilationInputs,
    ) -> Result<CompletedBuild, CompileFailure> {
        match self.build {
            ResolvedBuild::Returned(mut build) => {
                build.invocation.join_product_context(target, inputs)?;
                compiler.settle_package_build(build.invocation, build.returned)
            }
            ResolvedBuild::Settled(build) => {
                if build.package_inputs().source_inputs() != inputs.source_inputs() {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "settled helper Build differs from the final source graph",
                    )]
                    .into());
                }
                Ok(build)
            }
        }
    }

    pub fn complete(
        self,
        compiler: &mut CompilationSession,
        target: TargetProfile,
        inputs: PackageCompilationInputs,
    ) -> Result<PackageBuild, CompileFailure> {
        let mut completed = self.settle(compiler, target, inputs.clone())?;
        compiler.prepare_package_product(&mut completed, target, inputs, Default::default())
    }
}

pub(crate) fn settle_source_builds(
    builds: Vec<ResolvedPackageBuild>,
    compiler: &mut CompilationSession,
    profile: TargetProfile,
) -> Result<Vec<CompletedBuild>, ProjectSourceResolutionError> {
    let mut completed = Vec::with_capacity(builds.len());
    for build in builds {
        let inputs = build.package_inputs().clone();
        completed.push(
            build
                .settle(compiler, profile, inputs)
                .map_err(ProjectSourceResolutionError::Compilation)?,
        );
    }
    Ok(completed)
}

pub fn resolve_project_sources(
    session: &mut BuildSession,
    request: ProjectSourceRequest,
) -> Result<ResolvedProjectBuilds, ProjectSourceResolutionError> {
    use ProjectSourceResolutionError::Preparation;
    let ProjectSourceRequest {
        preparation,
        mut execution,
        collect_timings,
    } = request;
    let execution_profile = execution
        .profile
        .or_else(TargetProfile::host_if_supported)
        .ok_or_else(|| {
            ProjectSourceResolutionError::Source(
                "Build execution requires an admitted host profile".to_owned(),
            )
        })?;
    execution.profile = Some(execution_profile);

    let mut acquisitions = preparation.source_acquisitions().map_err(Preparation)?;
    // The caller's inventory bounds acquisition itself. Product discovery and
    // Build reads consume that same closed source tree from the first stage.
    let acquired = match execution
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.capture())
    {
        Some(package_compilation::BuildSnapshotCapture::Scoped(selected)) => {
            preparation.acquire_selected_root(selected)
        }
        Some(package_compilation::BuildSnapshotCapture::PackageInventory) | None => {
            preparation.acquire_root()
        }
    }
    .map_err(Preparation)?;
    let (resolved, builds) = resolve_acquired_sources(
        session,
        acquired,
        &preparation.entry_path(),
        &mut acquisitions,
        execution_profile,
        execution,
        collect_timings,
    )?;
    drop(acquisitions);
    let prepared = preparation
        .complete_source_resolution(resolved)
        .map_err(Preparation)?;
    Ok(ResolvedProjectBuilds {
        prepared,
        builds,
        execution_profile,
    })
}

pub(crate) fn resolve_acquired_sources(
    session: &mut BuildSession,
    acquired: AcquiredBuildSource,
    entry: &std::path::Path,
    acquisitions: &mut BuildSourceAcquisitions<'_>,
    execution_profile: TargetProfile,
    execution: package_compilation::BuildExecutionInputs,
    collect_timings: bool,
) -> Result<(ResolvedBuildSources, Vec<ResolvedPackageBuild>), ProjectSourceResolutionError> {
    let acquired = acquisitions
        .retain_root(acquired)
        .map_err(ProjectSourceResolutionError::Source)?;
    let mut scheduler = readiness::ReadinessScheduler::new(
        session,
        acquisitions,
        execution_profile,
        collect_timings,
    );
    let root = scheduler.execute_source(acquired, entry, execution)?;
    let resolved = scheduler.resolve_graph(root)?;
    let builds = scheduler.join_sources(&resolved)?;
    Ok((resolved, builds))
}

fn source_result(
    build: &mut ReturnedPackageBuild,
) -> Result<build_settings::BuildSourceResult<'_>, ProjectSourceResolutionError> {
    build
        .invocation
        .source_result(build.returned.as_mut())
        .map_err(ProjectSourceResolutionError::Compilation)
}
