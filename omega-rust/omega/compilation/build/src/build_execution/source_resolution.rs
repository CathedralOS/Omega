//! Execute acquired Build sources, reconcile their actual requests, retain work.
//!
//! Package management owns acquisition and graph validity. This application
//! owner runs the requested code and keeps each returned invocation through the
//! final source join. That work remains available to Product admission/review
//! rather than requiring Build replay. No source IR crosses this boundary.

mod readiness;

use crate::BuildSession;
use crate::build_execution::ReturnedPackageBuild;
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

/// Inputs known before the anonymous root executes. Product targets and
/// accepted package bindings join after its declaration; input inventory and
/// required outputs and resource sponsors cannot wait until then. Execution
/// inputs belong only to the root activation; dependencies use session resources.
pub struct ProjectSourceRequest {
    pub preparation: LocalProjectPreparation,
    pub execution: package_compilation::BuildExecutionInputs,
    pub collect_timings: bool,
}

/// Source acquisition and returned Build work share one lifetime. The roster
/// is discovery-ordered, not dependency completion order; consumers schedule
/// its exact package identities using the resolved inputs, never source paths.
pub struct ResolvedProjectBuilds {
    pub prepared: PreparedLocalProject,
    pub builds: Vec<ResolvedPackageBuild>,
    pub execution_profile: TargetProfile,
}

/// A Build whose complete request ledger has joined its acquired source graph.
/// Accepted grants and generated sources remain explicit later inputs. This is
/// not a portable compiled product and cannot be cloned or replayed.
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

    /// Settle the original execution before independently admitted Product
    /// contexts are prepared. No Product is typed and no live grant is retained.
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

    /// Complete the original invocation after package admission has joined
    /// generated inputs and any accepted policy. The Product target is chosen
    /// here, independently of the discovery/Build host. Compiler completion
    /// compares the graph with its executed receipt before Product typing.
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

/// Review consumes Product contexts from the same settled source roster.
/// Settlement joins each original activation; it neither replays Build nor
/// prepares a Product. Commands and inspection share this completion step.
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

/// Resolve actual dependency requests without static Build projection.
/// The shared execution owner is also used by ordinary compilation and review.
/// This migration entry retains normal returns for subsequent admission; it
/// does not authorize effects, refresh locks, or realize native dependencies.
/// Named Git/workspace selection still requires executed workspace declarations;
/// it never falls back to static declaration projection here.
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
    let acquired = preparation.acquire_root().map_err(Preparation)?;
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

/// One execution/discovery loop for live projects and staged package commands.
/// The caller keeps its transaction and acquisition policy; this owner retains
/// the exact activations whose declarations produced the resolved source graph.
pub(crate) fn resolve_acquired_sources(
    session: &mut BuildSession,
    acquired: AcquiredBuildSource,
    entry: &std::path::Path,
    acquisitions: &mut BuildSourceAcquisitions<'_>,
    execution_profile: TargetProfile,
    execution: package_compilation::BuildExecutionInputs,
    collect_timings: bool,
) -> Result<(ResolvedBuildSources, Vec<ResolvedPackageBuild>), ProjectSourceResolutionError> {
    acquisitions
        .retain_root(&acquired)
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

fn normal_return(
    build: &ReturnedPackageBuild,
) -> Result<&build_evaluation::ReturnedBuild, ProjectSourceResolutionError> {
    build.returned.as_ref().ok_or_else(|| {
        ProjectSourceResolutionError::Source(
            "acquired source has no executed Build declaration".into(),
        )
    })
}
