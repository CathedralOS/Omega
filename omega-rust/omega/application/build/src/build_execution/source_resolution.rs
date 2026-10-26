//! Execute acquired Build sources, reconcile their actual requests, retain work.
//!
//! Package management owns acquisition and graph validity. This application
//! owner runs the requested code and keeps each returned invocation through the
//! final source join. That work remains available to Product admission/review
//! rather than requiring Build replay. No source IR crosses this boundary.

use crate::BuildSession;
use crate::build_execution::{ReturnedPackageBuild, execute_returned_build};
use compiler::{CompilationSession, CompileFailure, CompletedBuild, PackageBuild};
use package_compilation::PackageCompilationInputs;
use package_manager::operations::{
    LocalProjectPreparation, PrepareLocalProjectError, PreparedLocalProject,
};
use package_manager::resolution::source::{
    AcquiredBuildSource, BuildSourceAcquisitions, ResolvedBuildSources,
};
use package_manager::resolution::source::{BuildSourceAcquisition, ExecutedSourceResolution};
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
/// required outputs cannot wait until then. This snapshot belongs only to the
/// root activation; dependency activations never inherit its named inputs.
pub struct ProjectSourceRequest {
    pub preparation: LocalProjectPreparation,
    pub execution_profile: TargetProfile,
    pub snapshot: Option<build_evaluation::BuildSnapshotRequest>,
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
    build: ReturnedPackageBuild,
    inputs: PackageCompilationInputs,
}

struct CatalogMemberBuild {
    capture: package_compilation::CapturedLocalSource,
    build: ReturnedPackageBuild,
}

impl ResolvedPackageBuild {
    pub fn package_inputs(&self) -> &PackageCompilationInputs {
        &self.inputs
    }

    /// Settle the original execution before independently admitted Product
    /// contexts are prepared. No Product is typed and no live grant is retained.
    pub fn settle(
        mut self,
        compiler: &mut CompilationSession,
        target: TargetProfile,
        inputs: PackageCompilationInputs,
    ) -> Result<CompletedBuild, CompileFailure> {
        self.build.invocation.join_product_context(target, inputs)?;
        let ReturnedPackageBuild {
            invocation,
            returned,
        } = self.build;
        compiler.settle_package_build(invocation, returned)
    }

    /// Complete the original invocation after package admission has joined
    /// generated inputs and any accepted policy. The Product target is chosen
    /// here, independently of the discovery/Build host. Compiler completion
    /// compares the graph with its executed receipt before Product typing.
    pub fn complete(
        mut self,
        compiler: &mut CompilationSession,
        target: TargetProfile,
        inputs: PackageCompilationInputs,
    ) -> Result<PackageBuild, CompileFailure> {
        self.build.invocation.join_product_context(target, inputs)?;
        self.build.complete(compiler)
    }
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
        execution_profile,
        snapshot,
        collect_timings,
    } = request;

    let mut acquisitions = preparation.source_acquisitions().map_err(Preparation)?;
    let acquired = preparation.acquire_root().map_err(Preparation)?;
    let (resolved, builds) = resolve_acquired_sources(
        session,
        acquired,
        &preparation.entry_path(),
        &mut acquisitions,
        execution_profile,
        snapshot,
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
    snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    collect_timings: bool,
) -> Result<(ResolvedBuildSources, Vec<ResolvedPackageBuild>), ProjectSourceResolutionError> {
    use ProjectSourceResolutionError::{Compilation, Resources, Source};
    let mut request = session
        .packages
        .source_build_request(acquired.source_capture(), entry, execution_profile)
        .map_err(Resources)?;
    if snapshot.is_some() {
        request.build.snapshot = snapshot;
    }
    request.collect_timings = collect_timings;
    let root = execute_returned_build(&mut session.compiler, request).map_err(Compilation)?;
    let declared = acquired
        .declare_build(normal_return(&root)?)
        .map_err(Source)?;
    let root_identity = declared.identity();
    let mut pending = vec![(root_identity, root)];
    let mut catalog_builds: Vec<CatalogMemberBuild> = Vec::new();
    let mut resolution = declared.resolve().map_err(Source)?;
    let resolved = 'sources: loop {
        match resolution {
            ExecutedSourceResolution::Complete(resolved) => break resolved,
            ExecutedSourceResolution::Request(request) => {
                let mut acquisition = acquisitions.acquire(request).map_err(Source)?;
                let acquired = loop {
                    match acquisition {
                        BuildSourceAcquisition::Execute(acquired) => break acquired,
                        BuildSourceAcquisition::Reused(next) => {
                            resolution = next;
                            continue 'sources;
                        }
                        BuildSourceAcquisition::Workspace(workspace) => {
                            let (members, declarations, builds) = execute_workspace_catalog(
                                session,
                                workspace,
                                execution_profile,
                                collect_timings,
                            )?;
                            catalog_builds.extend(builds);
                            acquisition = acquisitions
                                .complete_workspace(members, declarations)
                                .map_err(Source)?;
                        }
                    }
                };
                let capture = acquired.source_capture();
                let build = match catalog_builds
                    .iter()
                    .position(|build| &build.capture == capture)
                {
                    Some(position) => catalog_builds.swap_remove(position).build,
                    None => {
                        let mut request = session
                            .packages
                            .source_build_request(
                                capture,
                                &acquired.entry_path(),
                                execution_profile,
                            )
                            .map_err(Resources)?;
                        request.collect_timings = collect_timings;
                        execute_returned_build(&mut session.compiler, request)
                            .map_err(Compilation)?
                    }
                };
                let (identity, next) = acquired.complete(normal_return(&build)?).map_err(Source)?;
                pending.push((identity, build));
                resolution = next;
            }
        }
    };

    let mut builds = Vec::with_capacity(pending.len());
    for (identity, mut build) in pending {
        let returned = build
            .returned
            .as_mut()
            .ok_or_else(|| Source("acquired source has no executed Build declaration".into()))?;
        let inputs = resolved.join_build(identity, returned).map_err(Source)?;
        builds.push(ResolvedPackageBuild { build, inputs });
    }
    Ok((resolved, builds))
}

/// Root catalog obligations settle before membership is used. Members run only
/// far enough to observe their actual normal-return names; selected members keep
/// those same activations for dependency settlement and Product preparation.
fn execute_workspace_catalog(
    session: &mut BuildSession,
    workspace: package_manager::resolution::source::AcquiredWorkspaceBuild,
    profile: TargetProfile,
    collect_timings: bool,
) -> Result<
    (
        package_manager::resolution::source::AcquiredWorkspaceMembers,
        Vec<compiler::SourceBuildDeclaration>,
        Vec<CatalogMemberBuild>,
    ),
    ProjectSourceResolutionError,
> {
    use ProjectSourceResolutionError::{Compilation, Resources, Source};
    let mut request = session
        .packages
        .source_build_request(workspace.source_capture(), &workspace.entry_path(), profile)
        .map_err(Resources)?;
    request.collect_timings = collect_timings;
    let ReturnedPackageBuild {
        invocation,
        returned,
    } = execute_returned_build(&mut session.compiler, request).map_err(Compilation)?;
    let returned =
        returned.ok_or_else(|| Source("workspace catalog has no executed declaration".into()))?;
    let root = session
        .compiler
        .settle_source_catalog(invocation, returned)
        .map_err(Compilation)?;
    let members = workspace.members(root).map_err(Source)?;
    let mut declarations = Vec::new();
    let mut builds = Vec::new();
    for (capture, entry) in members.source_entries() {
        let mut request = session
            .packages
            .source_build_request(capture, &entry, profile)
            .map_err(Resources)?;
        request.collect_timings = collect_timings;
        let build = execute_returned_build(&mut session.compiler, request).map_err(Compilation)?;
        let declaration = build
            .invocation
            .observe_declaration(normal_return(&build)?)
            .map_err(Compilation)?;
        declarations.push(declaration);
        builds.push(CatalogMemberBuild {
            capture: capture.clone(),
            build,
        });
    }
    Ok((members, declarations, builds))
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
