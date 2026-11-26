//! Prepare a project by resolving its package dependencies and asking Build to execute each declaration.
mod services;
mod stages;
mod successor_inputs;
mod workspace_check;
pub use workspace_check::{discover_check_root, open_check_subjects};
#[cfg(all(test, unix))]
mod workspace_tests;

use crate::resolution::source::{
    AcquiredBuildSource, AcquiredWorkspaceBuild, BuildSourceAcquisition, BuildSourceAcquisitions,
    CatalogInput, CatalogPreparation, DeclaredBuildSource, ExecutedSourceResolution,
    ResolvedBuildSources, WorkspaceMemberObservation,
};
use crate::{PackageBuildSession, PackageCommandError};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use package_compilation::{BuildSnapshotRequest, PackageCompilationInputs};
use project_build::ReturnedPlanningStage;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use target::TargetProfile;

/// The original root Build has joined its own requests to this graph. Ordinary
/// compilation can now admit the graph against the project lock and open its Product.
struct ResolvedProject {
    sources: ResolvedBuildSources,
    root: ReturnedPlanningStage,
    inputs: PackageCompilationInputs,
}

/// Settled Build executions from one package transaction. Each Product opens
/// against the exact source graph its Build acquired, with its occurrence's
/// target handoffs and semantic bindings.
pub(crate) struct PackageReviewBuilds(Vec<project_build::SettledDependency>);

/// A resolved package graph and the original Build returns that produced it.
pub struct ResolvedPackageProject {
    pub(crate) sources: ResolvedBuildSources,
    pub(crate) builds: PackageReviewBuilds,
}

impl PackageReviewBuilds {
    pub(crate) fn product(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Result<project_build::PlannedProduct, build_evaluation_result::CompileFailure> {
        let source = inputs.source_inputs();
        let mut matching = self.0.iter().filter(|settled| {
            settled.inputs().root() == inputs.root() && settled.inputs().source_inputs() == source
        });
        let build = matching.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "package review has no settled Build for its exact acquired sources",
            )]
        })?;
        if matching.next().is_some() {
            return Err(vec![Diagnostic::error(
                "package review has multiple Build executions for the same source occurrence",
            )]
            .into());
        }
        build.open_product(inputs.clone())
    }
}

/// Execute a package transaction's root Build and settle the dependency graph it requested.
pub fn resolve_build_sources(
    session: &mut PackageBuildSession,
    acquired: AcquiredBuildSource,
    acquisitions: &mut BuildSourceAcquisitions<'_>,
    snapshot: Option<BuildSnapshotRequest>,
) -> Result<ResolvedPackageProject, PackageCommandError> {
    let profile = TargetProfile::host_if_supported()
        .ok_or_else(|| "package Build execution requires a supported host".to_owned())?;
    let acquired = acquisitions.retain_root(acquired)?;
    let mut host = SourceHost {
        session,
        acquisitions,
        profile,
        timings: false,
        observed: Vec::new(),
        active: Vec::new(),
        settled: Vec::new(),
    };
    let result = host.resolve(&acquired.entry_path(), acquired, snapshot);
    if let Err(error) = &result {
        host.session
            .record_source_completion(error.source_completion_accounting().clone());
    }
    let project = result.map_err(|error| {
        error.with_source_completion_accounting(host.session.source_completion_accounting().clone())
    })?;
    let root = project
        .root
        .settle_dependency(project.inputs)
        .map_err(|failure| {
            let error = PackageCommandError::from(failure);
            host.session
                .record_source_completion(error.source_completion_accounting().clone());
            error.with_source_completion_accounting(
                host.session.source_completion_accounting().clone(),
            )
        })?;
    host.session
        .record_source_completion(root.evaluated().source_completion_accounting.clone());
    let mut builds: Vec<_> = host
        .settled
        .into_iter()
        .map(|dependency| dependency.build)
        .collect();
    builds.push(root);
    Ok(ResolvedPackageProject {
        sources: project.sources,
        builds: PackageReviewBuilds(builds),
    })
}

struct SettledSource {
    source: AcquiredBuildSource,
    declaration: DeclaredBuildSource,
    build: project_build::SettledDependency,
    graph: ResolvedBuildSources,
}

struct PreparedRequest {
    settled: usize,
    acquisition: AcquiredBuildSource,
}

struct ExecutedBuild {
    preparations: Vec<(
        build_declarations::BuildEntryPath,
        Vec<successor_inputs::PreparedInput>,
    )>,
    stage: ReturnedPlanningStage,
    /// Each issued request uses a purpose-local ordinal. Alias lookup can inspect
    /// several candidates before the Build actually selects one.
    requests: BTreeMap<(DependencyPurpose, usize), PreparedRequest>,
}

impl ExecutedBuild {
    fn catalog_inputs(
        &mut self,
        settled: &[SettledSource],
    ) -> Result<Vec<CatalogInput>, PackageCommandError> {
        let mut inputs = Vec::new();
        for (ordinal, (successor, dependencies)) in self.preparations.iter().enumerate() {
            for dependency in dependencies {
                let selected = &dependency.prepared;
                let supplier = &settled[selected.settled];
                let graph = supplier
                    .graph
                    .rooted_at(&supplier.declaration, &selected.acquisition)?;
                inputs.push(CatalogInput {
                    preparation: Some(CatalogPreparation {
                        ordinal,
                        successor: successor.clone(),
                    }),
                    purpose: dependency.purpose,
                    ordinal: dependency.ordinal,
                    request: dependency.request.clone(),
                    graph: Arc::new(graph),
                });
            }
        }
        let result = self.stage.source_result()?;
        let issued =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                result.requests(),
            )?;
        for demand in result.demanded_sources() {
            let request = self
                .requests
                .get(&(demand.purpose, demand.request_ordinal))
                .ok_or_else(|| {
                    "workspace member demanded a dependency that was never prepared".to_owned()
                })?;
            let provider = &settled[request.settled];
            if provider.declaration.identity() != demand.package {
                return Err("workspace catalog demanded another dependency instance"
                    .to_owned()
                    .into());
            }
            let registration = issued
                .requests(demand.purpose)
                .get(demand.request_ordinal)
                .ok_or("catalog provider has no issued request".to_owned())?;
            inputs.push(CatalogInput {
                preparation: None,
                purpose: demand.purpose,
                ordinal: demand.request_ordinal,
                request: registration.clone(),
                graph: Arc::new(
                    provider
                        .graph
                        .rooted_at(&provider.declaration, &request.acquisition)?,
                ),
            });
        }
        Ok(inputs)
    }

    fn observe(
        &mut self,
        source: &AcquiredBuildSource,
        settled: &[SettledSource],
    ) -> Result<WorkspaceMemberObservation, PackageCommandError> {
        let inputs = self.catalog_inputs(settled)?;
        let declaration = self.stage.observe_source_declaration()?;
        let result = self.stage.source_result()?;
        Ok(WorkspaceMemberObservation::new(
            source,
            declaration,
            &result,
            inputs,
        )?)
    }
}

struct SourceHost<'session, 'policy> {
    session: &'session mut PackageBuildSession,
    acquisitions: &'session mut BuildSourceAcquisitions<'policy>,
    profile: TargetProfile,
    timings: bool,
    // Workspace members are executed to read their names. Retain their actual returns
    // so selection can settle those same invocations without charging for a second run.
    observed: Vec<(AcquiredBuildSource, ExecutedBuild)>,
    active: Vec<AcquiredBuildSource>,
    settled: Vec<SettledSource>,
}

impl SourceHost<'_, '_> {
    fn execute(
        &mut self,
        entry: &Path,
        acquired: &AcquiredBuildSource,
        snapshot: Option<BuildSnapshotRequest>,
        dependency: bool,
        libraries: &[BuildLibraryRequest],
    ) -> Result<ExecutedBuild, PackageCommandError> {
        self.acquisitions
            .admit_build_descent(&self.active, acquired)?;
        self.acquisitions.admit_locked_activation(acquired)?;
        self.active.push(acquired.clone());
        let result = self.execute_stages(entry, acquired, snapshot, dependency, libraries);
        self.active.pop();
        result
    }

    fn resolve(
        &mut self,
        entry: &Path,
        acquired: AcquiredBuildSource,
        snapshot: Option<BuildSnapshotRequest>,
    ) -> Result<ResolvedProject, PackageCommandError> {
        let root = self.execute(entry, &acquired, snapshot, false, &[])?;
        self.join(acquired, root.stage)
    }

    fn join(
        &mut self,
        acquired: AcquiredBuildSource,
        mut root: ReturnedPlanningStage,
    ) -> Result<ResolvedProject, PackageCommandError> {
        let declaration = acquired.declare_build(root.returned())?;
        let root_identity = declaration.identity();
        let mut returns = Vec::<(
            AcquiredBuildSource,
            DeclaredBuildSource,
            ReturnedPlanningStage,
        )>::new();
        let mut reused = Vec::<usize>::new();
        let mut next = declaration.clone().resolve()?;
        loop {
            let pending = match next {
                ExecutedSourceResolution::Complete(graph) => {
                    for (acquired, declaration, mut stage) in returns {
                        let inputs = graph.join_source_result(
                            declaration.identity(),
                            &mut stage.source_result()?,
                        )?;
                        let settled = stage.settle_dependency(inputs)?;
                        self.session.record_source_completion(
                            settled.evaluated().source_completion_accounting.clone(),
                        );
                        self.settled.push(SettledSource {
                            source: acquired,
                            declaration,
                            build: settled,
                            graph: graph.clone(),
                        });
                    }
                    for i in reused {
                        let settled = &self.settled[i];
                        let current = graph.compilation_inputs(settled.declaration.identity())?;
                        if current.source_inputs() != settled.build.inputs().source_inputs() {
                            return Err("settled helper Build differs from the final source graph"
                                .to_owned()
                                .into());
                        }
                    }
                    let inputs =
                        graph.join_source_result(root_identity, &mut root.source_result()?)?;
                    return Ok(ResolvedProject {
                        sources: graph,
                        root,
                        inputs,
                    });
                }
                ExecutedSourceResolution::Request(pending) => pending,
            };
            let requester = std::iter::once(&declaration)
                .chain(returns.iter().map(|(_, declared, _)| declared))
                .chain(self.settled.iter().map(|settled| &settled.declaration))
                .find(|declared| declared.identity() == pending.requester())
                .ok_or_else(|| {
                    "source resolution lost the requesting Build declaration".to_owned()
                })?;
            let frozen = pending
                .frozen_binding(requester)
                .map(|binding| {
                    self.frozen_library(binding.library())
                        .map(|index| self.settled[index].source.clone())
                })
                .transpose()?;
            let mut acquisition = self.acquisitions.acquire_or_reuse(pending, frozen)?;
            next = loop {
                match acquisition {
                    BuildSourceAcquisition::Reused(next) => break next,
                    BuildSourceAcquisition::Workspace { pending, workspace } => {
                        let selected = self.select_workspace(workspace)?;
                        acquisition = pending.select(selected)?;
                    }
                    BuildSourceAcquisition::Execute(dependency) => {
                        let acquired = dependency.acquired_source();
                        if let Some(i) = self
                            .settled
                            .iter()
                            .position(|settled| settled.source.same_source(acquired))
                        {
                            let settled = &self.settled[i];
                            acquired.verify_reused_capture(&settled.source)?;
                            let (_, next) = dependency.complete_declared(&settled.declaration)?;
                            reused.push(i);
                            break next;
                        }
                        let acquired = acquired.clone();
                        let stage = self.member_or_execute(&acquired)?.stage;
                        let declaration = acquired.declare_build(stage.returned())?;
                        let (_, next) = dependency.complete_declared(&declaration)?;
                        returns.push((acquired, declaration, stage));
                        break next;
                    }
                }
            };
        }
    }

    fn member_or_execute(
        &mut self,
        acquired: &AcquiredBuildSource,
    ) -> Result<ExecutedBuild, PackageCommandError> {
        if let Some(i) = self
            .observed
            .iter()
            .position(|(source, _)| source.same_source(acquired))
        {
            let (source, stage) = self.observed.swap_remove(i);
            acquired.verify_reused_capture(&source)?;
            Ok(stage)
        } else {
            self.execute(&acquired.entry_path(), acquired, None, true, &[])
        }
    }

    fn select_workspace(
        &mut self,
        workspace: AcquiredWorkspaceBuild,
    ) -> Result<AcquiredBuildSource, PackageCommandError> {
        let mut root = self.execute(
            &workspace.entry_path(),
            workspace.acquired_source(),
            None,
            true,
            &[],
        )?;
        let inputs = root.catalog_inputs(&self.settled)?;
        let declaration = root.stage.settle_workspace_declaration()?;
        let members = self
            .acquisitions
            .workspace_members(workspace, declaration, inputs)?;
        let mut observations = Vec::new();
        for source in members.sources() {
            let mut member = self.member_or_execute(source)?;
            observations.push(member.observe(source, &self.settled)?);
            self.observed.push((source.clone(), member));
        }
        Ok(self
            .acquisitions
            .complete_workspace(members, observations)?)
    }
}

type Deployment = Result<Rc<project_build::PlannedProduct>, Vec<Diagnostic>>;

/// The Product prepared for each requested deployment target, and the
/// Build rechecks each may still owe. Targets that share one Product share
/// its rechecks.
pub struct ProjectProducts(
    Vec<(TargetProfile, Deployment)>,
    Vec<(TargetProfile, Rc<RefCell<OwedRechecks>>)>,
    CompileTimings,
);

/// The rechecks one deployment owes, until they are settled once.
enum OwedRechecks {
    Running(services::DependencyRechecks),
    Settled(Result<(), Vec<Diagnostic>>),
}

impl ProjectProducts {
    fn shared(
        profiles: &[TargetProfile],
        planned: project_build::PlannedProduct,
        rechecks: services::DependencyRechecks,
        open_timings: CompileTimings,
    ) -> Self {
        let planned = Rc::new(planned);
        let owed = Rc::new(RefCell::new(OwedRechecks::Running(rechecks)));
        Self(
            profiles
                .iter()
                .map(|&target| (target, Ok(Rc::clone(&planned))))
                .collect(),
            profiles
                .iter()
                .map(|&target| (target, Rc::clone(&owed)))
                .collect(),
            open_timings,
        )
    }

    /// Open the one Product a root without dependencies shares across its
    /// targets. With `alongside`, the root's own Build recheck may still be
    /// running when this returns, as in [`open_check_subjects`].
    fn open_shared(
        profiles: &[TargetProfile],
        root: ReturnedPlanningStage,
        inputs: Option<&PackageCompilationInputs>,
        alongside: bool,
        open_timings: CompileTimings,
    ) -> Result<Self, build_evaluation_result::CompileFailure> {
        if !(alongside && project_build::rechecks_outlive_caller()) {
            return Ok(Self::shared(
                profiles,
                root.open_product(inputs)?,
                Default::default(),
                open_timings,
            ));
        }
        let (planned, own) = root.open_pending(inputs)?.release_for_check();
        Ok(Self::shared(
            profiles,
            planned,
            services::DependencyRechecks::own(own),
            open_timings,
        ))
    }

    pub fn open_timings(&self) -> &CompileTimings {
        &self.2
    }

    /// Wait for the Build rechecks the Product for `target` still owes: its
    /// own and its dependencies'. Their refusal is that Product's result,
    /// ahead of any refusal of its own, so report nothing about the Product
    /// before this returns. The Build work they measured is returned once,
    /// by the first call.
    pub fn settle_rechecks(
        &self,
        target: TargetProfile,
    ) -> Result<CompileTimings, Vec<Diagnostic>> {
        let Some((_, owed)) = self.1.iter().find(|(profile, _)| *profile == target) else {
            return Ok(CompileTimings::default());
        };
        let mut owed = owed.borrow_mut();
        let settled = match std::mem::replace(&mut *owed, OwedRechecks::Settled(Ok(()))) {
            OwedRechecks::Running(rechecks) => rechecks
                .settle()
                .map_err(|failure| failure.into_diagnostics()),
            OwedRechecks::Settled(result) => result.map(|()| CompileTimings::default()),
        };
        *owed = OwedRechecks::Settled(settled.as_ref().map(|_| ()).map_err(Clone::clone));
        settled
    }

    pub fn select(
        &self,
        target: TargetProfile,
    ) -> Result<&project_build::PlannedProduct, Vec<Diagnostic>> {
        self.0
            .iter()
            .find(|(profile, _)| *profile == target)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "project did not prepare target {}",
                    target.target_name()
                ))]
            })?
            .1
            .as_deref()
            .map_err(Clone::clone)
    }
}

fn planning_timings(enabled: bool) -> CompileTimings {
    if enabled {
        CompileTimings::enabled()
    } else {
        CompileTimings::default()
    }
}

/// The root invoker's selection. The package manager resolves the source before entering the receiving Build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildLibraryRequest {
    pub alias: build_declarations::ProjectName,
    pub source: build_declarations::BuildDependencySource,
}

/// The initial file and any Build libraries explicitly supplied by its invoker.
pub enum ProjectInput<'a> {
    File(&'a Path),
    WithBuildLibraries {
        file: &'a Path,
        libraries: &'a [BuildLibraryRequest],
    },
}

impl ProjectInput<'_> {
    fn path(&self) -> &Path {
        match self {
            Self::File(path) | Self::WithBuildLibraries { file: path, .. } => path,
        }
    }
}

/// Prepare a project for a single deployment target.
pub fn open_product(
    root: &Path,
    profile: TargetProfile,
    offline: bool,
    build_directory: &Path,
    snapshot: Option<BuildSnapshotRequest>,
) -> Deployment {
    open_products(
        ProjectInput::File(root),
        &[profile],
        offline,
        false,
        build_directory,
        snapshot,
    )?
    .0
    .into_iter()
    .next()
    .expect("one deployment was requested")
    .1
}

/// Resolve and execute the authored Build once for the entire deployment set.
/// The returned Product keeps the same source capture, grant joins and Build return.
/// Every dependency Build recheck has passed before it returns.
pub fn open_products(
    input: ProjectInput<'_>,
    profiles: &[TargetProfile],
    offline: bool,
    timings: bool,
    build_directory: &Path,
    snapshot: Option<BuildSnapshotRequest>,
) -> Result<ProjectProducts, Vec<diagnostics::Diagnostic>> {
    open_products_with(
        input,
        profiles,
        offline,
        timings,
        build_directory,
        snapshot,
        OpenPurpose::Product,
    )?
    .into_product()
}

/// A catalog may produce check subjects only when the invocation requests a check.
enum OpenPurpose<'a> {
    Product,
    Check,
    Member(
        &'a AcquiredBuildSource,
        &'a Arc<crate::resolution::source::LocalWorkspaceMembers>,
    ),
}

enum OpenedRoot {
    Product(ProjectProducts, Option<build_declarations::ProjectName>),
    Catalog(build_execution::SourceBuildDeclaration),
}

impl OpenedRoot {
    fn into_product(self) -> Result<ProjectProducts, Vec<Diagnostic>> {
        match self {
            Self::Product(products, _) => Ok(products),
            Self::Catalog(_) => Err(vec![Diagnostic::error(
                "a workspace catalog has no Product",
            )]),
        }
    }
}

const OPEN_PREPARE: StageMeta = StageMeta::new("prepare", "ProjectInput", "LocalPreparation");
const OPEN_ACQUIRE_ROOT: StageMeta =
    StageMeta::new("acquire root", "LocalPreparation", "CapturedRoot");
const OPEN_EXECUTE_BUILD: StageMeta = StageMeta::new("execute Build", "CapturedRoot", "BuildStage");
const OPEN_RESOLVE_SOURCES: StageMeta =
    StageMeta::new("resolve sources", "BuildStage", "ProjectSources");
const OPEN_DEPENDENCY_HANDOFF: StageMeta =
    StageMeta::new("dependency handoff", "ProjectSources", "PlannedProducts");
const OPEN_DISPOSE_SESSION: StageMeta =
    StageMeta::new("dispose session", "PlannedProducts", "ProjectProducts");

fn open_products_with(
    input: ProjectInput<'_>,
    profiles: &[TargetProfile],
    offline: bool,
    timings: bool,
    build_directory: &Path,
    snapshot: Option<BuildSnapshotRequest>,
    purpose: OpenPurpose<'_>,
) -> Result<OpenedRoot, Vec<diagnostics::Diagnostic>> {
    let alongside = !matches!(purpose, OpenPurpose::Product);
    use crate::operations::begin_local_project_preparation;
    use package_compilation::build_execution::BuildSnapshotCapture;
    let mut open_timings = planning_timings(timings);
    let error = |error: PackageCommandError| {
        error
            .compilation_failure()
            .cloned()
            .map(|failure| failure.into_diagnostics())
            .unwrap_or_else(|| vec![diagnostics::Diagnostic::error(error.to_string())])
    };
    let profile = TargetProfile::host_if_supported().ok_or_else(|| {
        vec![diagnostics::Diagnostic::error(
            "project Build execution needs a supported host",
        )]
    })?;
    let root = input.path();
    let preparation = open_timings
        .record_result(OPEN_PREPARE, || match purpose {
            OpenPurpose::Member(source, _) => {
                crate::operations::LocalProjectPreparation::for_unlocked_member(
                    source, profiles, offline,
                )
            }
            _ => begin_local_project_preparation(root, profiles, offline),
        })
        .map_err(|e| error(e.into()))?;
    let mut session = PackageBuildSession::create(&build_directory.join("package-manager"))
        .map_err(|e| error(e.into()))?;
    let result = (|| -> Result<_, PackageCommandError> {
        let rejected: Vec<_> = preparation
            .requested_target_errors()
            .map(|(target, reason)| (target, Err(vec![Diagnostic::error(reason.to_string())])))
            .collect();
        if rejected.len() == profiles.len() {
            return Ok(OpenedRoot::Product(
                ProjectProducts(rejected, Vec::new(), open_timings.clone()),
                None,
            ));
        }
        let step = timings.then(std::time::Instant::now);
        let selected = match snapshot.as_ref().map(|snapshot| snapshot.capture()) {
            Some(BuildSnapshotCapture::Scoped(selected)) => Some(selected),
            _ => None,
        };
        let acquired = match (&purpose, selected) {
            (OpenPurpose::Member(source, _), _) => (*source).clone(),
            (_, Some(selection)) => preparation.acquire_selected_root(selection)?,
            (_, None) => preparation.acquire_root()?,
        };
        let mut acquisitions = preparation.source_acquisitions()?;
        if let OpenPurpose::Member(_, members) = purpose {
            acquisitions.local_members = Some(Arc::clone(members));
        }
        let acquired = acquisitions.retain_root(acquired)?;
        if let Some(started) = step {
            open_timings.add_completed(OPEN_ACQUIRE_ROOT, started.elapsed().as_micros());
        }
        let mut host = SourceHost {
            session: &mut session,
            acquisitions: &mut acquisitions,
            profile,
            timings,
            observed: Vec::new(),
            active: Vec::new(),
            settled: Vec::new(),
        };
        let entry = if matches!(purpose, OpenPurpose::Check)
            && root.file_name().is_some_and(|name| name == "main.omg")
        {
            acquired.entry_path()
        } else {
            preparation.entry_path()
        };
        let libraries = match input {
            ProjectInput::WithBuildLibraries { libraries, .. } => libraries,
            _ => &[],
        };
        let returned = open_timings
            .record_result(OPEN_EXECUTE_BUILD, || {
                host.execute(&entry, &acquired, snapshot, false, libraries)
            })?
            .stage;
        let name = match returned.returned().declaration() {
            Some(build_declarations::BuildDeclaration::Workspace(_)) => match purpose {
                OpenPurpose::Check => {
                    preparation.require_unlocked_anonymous()?;
                    return Ok(OpenedRoot::Catalog(
                        returned.settle_workspace_declaration()?,
                    ));
                }
                OpenPurpose::Member(..) => {
                    return Err("a workspace member cannot itself be a catalog"
                        .to_owned()
                        .into());
                }
                OpenPurpose::Product => {
                    return Err(
                        "a workspace declaration does not name a Product source owner"
                            .to_owned()
                            .into(),
                    );
                }
            },
            Some(build_declarations::BuildDeclaration::Package(package)) => {
                Some(package.name.clone())
            }
            Some(build_declarations::BuildDeclaration::Application(application)) => {
                Some(application.name.clone())
            }
            None if matches!(purpose, OpenPurpose::Member(..)) => {
                return Err(
                    "a workspace member must declare a package or application name"
                        .to_owned()
                        .into(),
                );
            }
            None => None,
        };
        if returned.returned().declaration().is_none() {
            preparation.require_unlocked_anonymous()?;
            return Ok(OpenedRoot::Product(
                ProjectProducts::open_shared(
                    profiles,
                    returned,
                    None,
                    alongside,
                    open_timings.clone(),
                )?,
                name,
            ));
        }
        let step = timings.then(std::time::Instant::now);
        let resolved = host.join(acquired, returned)?;
        let settled = host.settled;
        let project = preparation.complete_source_resolution(resolved.sources)?;
        // Source acquisition uses the lock's exact pins. Both ordinary checking and
        // --check --verify start from captured source, without installing recorded trust
        // admissions or a restricted-Build checkpoint. Runtime consent is a separate decision.
        let decisions: Vec<_> = profiles
            .iter()
            .copied()
            .map(|target| {
                (
                    target,
                    project
                        .accepted_deployment(target)
                        .map_err(|reason| vec![Diagnostic::error(reason)]),
                )
            })
            .collect();
        if let Some(started) = step {
            open_timings.add_completed(OPEN_RESOLVE_SOURCES, started.elapsed().as_micros());
        }
        if resolved
            .inputs
            .required_dependency_source_instances()
            .is_empty()
            && decisions
                .iter()
                .all(|(_, policy)| matches!(policy, Ok(None)))
        {
            return Ok(OpenedRoot::Product(
                ProjectProducts::open_shared(
                    profiles,
                    resolved.root,
                    Some(&resolved.inputs),
                    alongside,
                    open_timings.clone(),
                )?,
                name,
            ));
        }
        let step = timings.then(std::time::Instant::now);
        let root = resolved.root.settle_dependency(resolved.inputs)?;
        let mut owed = Vec::new();
        let products = decisions
            .into_iter()
            .map(|(target, decision)| {
                let product = decision.and_then(|accepted| {
                    let sources = services::HandoffSources {
                        profile,
                        settled: &settled,
                        accepted,
                    };
                    // A Build-host occurrence can carry different accepted bindings in different
                    // deployment sections even when its target and host are the same.
                    let mut dependencies = sources.deployment_products(&root, target);
                    let consent = accepted
                        .map(|policy| policy.root_permissions())
                        .transpose()
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    let (planned, rechecks) = sources
                        .dependency_product(
                            &root,
                            build_declarations::DependencyPurpose::Product,
                            target,
                            &mut dependencies,
                            alongside,
                        )
                        .map_err(|failure| error(failure.into()))?;
                    owed.push((
                        target,
                        Rc::new(RefCell::new(OwedRechecks::Running(rechecks))),
                    ));
                    let planned = planned
                        .with_dependency_timings(dependencies.take_timings())
                        .with_component_suppliers(dependencies.into_suppliers());
                    Ok(Rc::new(match consent {
                        Some(consent) => planned.with_runtime_consents(vec![consent]),
                        None => planned,
                    }))
                });
                (target, product)
            })
            .collect();
        if let Some(started) = step {
            open_timings.add_completed(OPEN_DEPENDENCY_HANDOFF, started.elapsed().as_micros());
        }
        Ok(OpenedRoot::Product(
            ProjectProducts(products, owed, open_timings.clone()),
            name,
        ))
    })();
    let (dispose_us, result) = if timings {
        let dispose_start = std::time::Instant::now();
        let disposed = session.dispose(result);
        (Some(dispose_start.elapsed().as_micros()), disposed)
    } else {
        (None, session.dispose(result))
    };
    let mut products = result.map_err(error)?;
    if let (Some(microseconds), OpenedRoot::Product(products, _)) = (dispose_us, &mut products) {
        products.2.add_completed(OPEN_DISPOSE_SESSION, microseconds);
    }
    Ok(products)
}

#[cfg(test)]
mod owed_recheck_tests {
    use super::{OwedRechecks, ProjectProducts};
    use artifacts::compile_timings::CompileTimings;
    use diagnostics::Diagnostic;
    use std::cell::RefCell;
    use std::rc::Rc;
    use target::TargetProfile;

    /// A refused dependency recheck stays its deployment's result however
    /// often it is settled, so the CLI can settle inside the check and again
    /// before reporting, and still report that refusal first.
    #[test]
    fn a_settled_recheck_refusal_is_returned_every_time() {
        let refused = vec![Diagnostic::error("dependency recheck refused")];
        let products = ProjectProducts(
            Vec::new(),
            vec![
                (
                    TargetProfile::LinuxX64,
                    Rc::new(RefCell::new(OwedRechecks::Settled(Err(refused.clone())))),
                ),
                (
                    TargetProfile::LinuxArm64,
                    Rc::new(RefCell::new(OwedRechecks::Running(Default::default()))),
                ),
            ],
            CompileTimings::default(),
        );
        for _ in 0..2 {
            assert_eq!(
                products
                    .settle_rechecks(TargetProfile::LinuxX64)
                    .unwrap_err(),
                refused
            );
            assert!(products.settle_rechecks(TargetProfile::LinuxArm64).is_ok());
        }
        // A deployment that owes no recheck settles at once.
        assert!(products.settle_rechecks(TargetProfile::MacosArm64).is_ok());
    }
}
