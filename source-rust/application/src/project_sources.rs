//! Execute acquired Build declarations and join their issuing activations to the resolved graph.
mod services;

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::{BuildSnapshotRequest, PackageCompilationInputs};
use package_manager::resolution::source::{
    AcquiredBuildSource, AcquiredWorkspaceBuild, BuildSourceAcquisition, BuildSourceAcquisitions,
    DeclaredBuildSource, ExecutedSourceResolution, ResolvedBuildSources,
    WorkspaceMemberObservation,
};
use package_manager::{PackageBuildSession, PackageCommandError};
use project_build::planning::{self, PlanningRequest, PlanningSources, ReturnedPlanningStage};
use std::path::Path;
use std::rc::Rc;
use target::TargetProfile;

/// The original root Build has joined its own requests to this graph. Ordinary
/// compilation can now admit the graph against the project lock and open its Product.
pub(crate) struct ResolvedProject {
    pub(crate) sources: ResolvedBuildSources,
    pub(crate) root: ReturnedPlanningStage,
    pub(crate) inputs: PackageCompilationInputs,
}

impl ResolvedProject {
    pub(crate) fn settle(
        self,
        session: &mut PackageBuildSession,
    ) -> Result<ResolvedBuildSources, PackageCommandError> {
        let evaluated = self.root.complete(Some(&self.inputs)).map_err(|failure| {
            let error = PackageCommandError::from(failure);
            session.record_source_completion(error.source_completion_accounting().clone());
            error.with_source_completion_accounting(session.source_completion_accounting().clone())
        })?;
        session.record_source_completion(evaluated.source_completion_accounting);
        Ok(self.sources)
    }
}

pub(crate) fn resolve(
    session: &mut PackageBuildSession,
    entry: &Path,
    acquired: AcquiredBuildSource,
    acquisitions: &mut BuildSourceAcquisitions<'_>,
    snapshot: Option<BuildSnapshotRequest>,
) -> Result<ResolvedProject, PackageCommandError> {
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
    let result = host.resolve(entry, acquired, snapshot);
    if let Err(error) = &result {
        host.session
            .record_source_completion(error.source_completion_accounting().clone());
    }
    result.map_err(|error| {
        error.with_source_completion_accounting(host.session.source_completion_accounting().clone())
    })
}

type SettledSource = (
    AcquiredBuildSource,
    DeclaredBuildSource,
    planning::SettledDependency,
);

struct SourceHost<'session, 'policy> {
    session: &'session mut PackageBuildSession,
    acquisitions: &'session mut BuildSourceAcquisitions<'policy>,
    profile: TargetProfile,
    timings: bool,
    // Workspace members are executed to read their names. Retain their actual returns
    // so selection can settle those same invocations without charging for a second run.
    observed: Vec<(AcquiredBuildSource, ReturnedPlanningStage)>,
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
    ) -> Result<ReturnedPlanningStage, PackageCommandError> {
        self.acquisitions
            .admit_build_descent(&self.active, acquired)?;
        self.acquisitions.admit_locked_activation(acquired)?;
        let execution = self
            .session
            .source_build_inputs(self.profile, snapshot, dependency)?;
        let stage = planning::prepare(
            PlanningRequest {
                root: entry,
                sources: PlanningSources::Captured(acquired.source_capture()),
                execution,
                artifact_only: false,
            },
            Default::default(),
            planning_timings(self.timings),
        )?;
        self.active.push(acquired.clone());
        let result = planning::interpret(
            stage,
            &mut services::SourceServices {
                host: self,
                acquired,
            },
        );
        self.active.pop();
        let stage = result?;
        self.session
            .record_source_completion(stage.source_completion_accounting().clone());
        Ok(stage)
    }

    fn resolve(
        &mut self,
        entry: &Path,
        acquired: AcquiredBuildSource,
        snapshot: Option<BuildSnapshotRequest>,
    ) -> Result<ResolvedProject, PackageCommandError> {
        let root = self.execute(entry, &acquired, snapshot, false)?;
        self.join(acquired, root)
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
        let mut next = declaration.resolve()?;
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
                        self.settled.push((acquired, declaration, settled));
                    }
                    for i in reused {
                        let (_, declaration, settled) = &self.settled[i];
                        let current = graph.compilation_inputs(declaration.identity())?;
                        if current.source_inputs() != settled.inputs().source_inputs() {
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
            let mut acquisition = self.acquisitions.acquire(pending)?;
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
                            .position(|(source, _, _)| source.same_source(acquired))
                        {
                            let (source, declaration, _) = &self.settled[i];
                            acquired.verify_reused_capture(source)?;
                            let (_, next) = dependency.complete_declared(declaration)?;
                            reused.push(i);
                            break next;
                        }
                        let acquired = acquired.clone();
                        let stage = self.member_or_execute(&acquired)?;
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
    ) -> Result<ReturnedPlanningStage, PackageCommandError> {
        if let Some(i) = self
            .observed
            .iter()
            .position(|(source, _)| source.same_source(acquired))
        {
            let (source, stage) = self.observed.swap_remove(i);
            acquired.verify_reused_capture(&source)?;
            Ok(stage)
        } else {
            self.execute(&acquired.entry_path(), acquired, None, true)
        }
    }

    fn select_workspace(
        &mut self,
        workspace: AcquiredWorkspaceBuild,
    ) -> Result<AcquiredBuildSource, PackageCommandError> {
        let root = self.execute(
            &workspace.entry_path(),
            workspace.acquired_source(),
            None,
            true,
        )?;
        let declaration = root.settle_workspace_declaration()?;
        let members = self
            .acquisitions
            .workspace_members(workspace, declaration)?;
        let mut observations = Vec::new();
        for source in members.sources() {
            let mut stage = self.member_or_execute(source)?;
            let declaration = stage.observe_source_declaration()?;
            observations.push(WorkspaceMemberObservation::new(
                source,
                declaration,
                &stage.source_result()?,
                Vec::new(),
            )?);
            self.observed.push((source.clone(), stage));
        }
        Ok(self
            .acquisitions
            .complete_workspace(members, observations)?)
    }
}

type Deployment = Result<Rc<planning::PlannedProduct>, Vec<Diagnostic>>;

pub(crate) struct ProjectProducts(Vec<(TargetProfile, Deployment)>);

impl ProjectProducts {
    fn shared(profiles: &[TargetProfile], planned: planning::PlannedProduct) -> Self {
        let planned = Rc::new(planned);
        Self(
            profiles
                .iter()
                .map(|&target| (target, Ok(Rc::clone(&planned))))
                .collect(),
        )
    }

    pub fn select(
        &self,
        target: TargetProfile,
    ) -> Result<&planning::PlannedProduct, Vec<Diagnostic>> {
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

pub(crate) fn open_product(
    root: &Path,
    profile: TargetProfile,
    offline: bool,
    build_directory: Option<&Path>,
    snapshot: Option<BuildSnapshotRequest>,
) -> Deployment {
    open_products(root, &[profile], offline, false, build_directory, snapshot)?
        .0
        .into_iter()
        .next()
        .expect("one deployment was requested")
        .1
}

/// Resolve and execute the authored Build once for the entire deployment set.
/// The returned Product keeps the same source capture, grant joins and Build return.
pub(crate) fn open_products(
    root: &Path,
    profiles: &[TargetProfile],
    offline: bool,
    timings: bool,
    build_directory: Option<&Path>,
    snapshot: Option<BuildSnapshotRequest>,
) -> Result<ProjectProducts, Vec<diagnostics::Diagnostic>> {
    use package_compilation::build_execution::BuildSnapshotCapture;
    use package_manager::operations::begin_local_project_preparation;
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
    let preparation =
        begin_local_project_preparation(root, profiles, offline).map_err(|e| error(e.into()))?;
    let workspace = project_build::output_directory::base(root, build_directory);
    let mut session = PackageBuildSession::create(&workspace.join("package-manager"))
        .map_err(|e| error(e.into()))?;
    let result = (|| -> Result<_, PackageCommandError> {
        if let Some(preparation) = &preparation {
            let rejected: Vec<_> = preparation
                .requested_target_errors()
                .map(|(target, reason)| (target, Err(vec![Diagnostic::error(reason.to_string())])))
                .collect();
            if rejected.len() == profiles.len() {
                return Ok(ProjectProducts(rejected));
            }
        }
        let selected = match snapshot.as_ref().map(|snapshot| snapshot.capture()) {
            Some(BuildSnapshotCapture::Scoped(selected)) => Some(selected),
            _ => None,
        };
        let Some(preparation) = preparation else {
            let capture = match selected {
                Some(selection) => {
                    package_compilation::capture_selected_anonymous_compilation_source(
                        root, selection,
                    )
                }
                None => package_compilation::capture_anonymous_compilation_source(root),
            }
            .map_err(|rows| {
                let mut text = String::new();
                let _ = diagnostics::write_diagnostics(&mut text, &rows);
                text
            })?;
            let stage = planning::prepare(
                PlanningRequest {
                    root,
                    sources: PlanningSources::Captured(&capture),
                    execution: session.source_build_inputs(profile, snapshot, false)?,
                    artifact_only: false,
                },
                Default::default(),
                planning_timings(timings),
            )?;
            return Ok(ProjectProducts::shared(
                profiles,
                planning::interpret(stage, &mut ())?.open_product(None)?,
            ));
        };
        let acquired = match selected {
            Some(selection) => preparation.acquire_selected_root(selection)?,
            None => preparation.acquire_root()?,
        };
        let mut acquisitions = preparation.source_acquisitions()?;
        let acquired = acquisitions.retain_root(acquired)?;
        let mut host = SourceHost {
            session: &mut session,
            acquisitions: &mut acquisitions,
            profile,
            timings,
            observed: Vec::new(),
            active: Vec::new(),
            settled: Vec::new(),
        };
        let entry = preparation.entry_path();
        let returned = host.execute(&entry, &acquired, snapshot, false)?;
        if returned.returned().declaration().is_none() {
            preparation.require_unlocked_anonymous()?;
            return Ok(ProjectProducts::shared(
                profiles,
                returned.open_product(None)?,
            ));
        }
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
        if resolved
            .inputs
            .required_dependency_source_instances()
            .is_empty()
            && decisions
                .iter()
                .all(|(_, policy)| matches!(policy, Ok(None)))
        {
            return Ok(ProjectProducts::shared(
                profiles,
                resolved.root.open_product(Some(&resolved.inputs))?,
            ));
        }
        let root = resolved.root.settle_dependency(resolved.inputs)?;
        Ok(ProjectProducts(
            decisions
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
                        let mut cache = Vec::new();
                        let consent = accepted
                            .map(|policy| policy.root_permissions())
                            .transpose()
                            .map_err(|reason| vec![Diagnostic::error(reason)])?;
                        let planned = sources
                            .dependency_product(
                                &root,
                                build_declarations::DependencyPurpose::Product,
                                target,
                                &mut cache,
                            )
                            .map_err(|failure| error(failure.into()))?;
                        Ok(Rc::new(match consent {
                            Some(consent) => planned.with_runtime_consents(vec![consent]),
                            None => planned,
                        }))
                    });
                    (target, product)
                })
                .collect(),
        ))
    })();
    session.dispose(result).map_err(error)
}

#[cfg(test)]
mod tests;
