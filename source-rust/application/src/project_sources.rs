//! Execute acquired Build declarations and join their issuing activations to the resolved graph.
use package_compilation::{BuildSnapshotRequest, PackageCompilationInputs};
use package_manager::resolution::source::{
    AcquiredBuildSource, AcquiredWorkspaceBuild, BuildSourceAcquisition, BuildSourceAcquisitions,
    ExecutedSourceResolution, ResolvedBuildSources, WorkspaceMemberObservation,
};
use package_manager::{PackageBuildSession, PackageCommandError};
use project_build::planning::{self, PlanningRequest, PlanningSources, ReturnedPlanningStage};
use std::path::Path;
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
        observed: Vec::new(),
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

struct SourceHost<'session, 'policy> {
    session: &'session mut PackageBuildSession,
    acquisitions: &'session mut BuildSourceAcquisitions<'policy>,
    profile: TargetProfile,
    // Workspace members are executed to read their names. Retain their actual returns
    // so selection can settle those same invocations without charging for a second run.
    observed: Vec<(AcquiredBuildSource, ReturnedPlanningStage)>,
}

impl SourceHost<'_, '_> {
    fn execute(
        &mut self,
        entry: &Path,
        acquired: &AcquiredBuildSource,
        snapshot: Option<BuildSnapshotRequest>,
        dependency: bool,
    ) -> Result<ReturnedPlanningStage, PackageCommandError> {
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
            Default::default(),
        )?;
        let stage = planning::interpret(stage, &mut ())?;
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
        let mut root = self.execute(entry, &acquired, snapshot, false)?;
        let declaration = acquired.declare_build(root.returned())?;
        let root_identity = declaration.identity();
        let mut returns = Vec::<(_, ReturnedPlanningStage)>::new();
        let mut next = declaration.resolve()?;
        loop {
            let pending = match next {
                ExecutedSourceResolution::Complete(graph) => {
                    for (subject, mut stage) in returns {
                        let inputs =
                            graph.join_source_result(subject, &mut stage.source_result()?)?;
                        let evaluated = stage.complete(Some(&inputs))?;
                        self.session
                            .record_source_completion(evaluated.source_completion_accounting);
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
                        let stage = self.member_or_execute(acquired)?;
                        let declaration = acquired.declare_build(stage.returned())?;
                        let (subject, next) = dependency.complete_declared(&declaration)?;
                        returns.push((subject, stage));
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
