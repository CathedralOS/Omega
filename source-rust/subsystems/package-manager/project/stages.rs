//! Follow the returned Build entries of one captured project until it declares its final role.
use super::{BuildLibraryRequest, ExecutedBuild, SourceHost, planning_timings, services};
use crate::PackageCommandError;
use crate::resolution::source::AcquiredBuildSource;
use package_compilation::BuildSnapshotRequest;
use project_build::{PlanningRequest, PlanningSources};
use std::path::Path;

impl SourceHost<'_, '_> {
    pub(super) fn execute_stages(
        &mut self,
        entry: &Path,
        acquired: &AcquiredBuildSource,
        mut snapshot: Option<BuildSnapshotRequest>,
        dependency: bool,
        supplied: &[BuildLibraryRequest],
    ) -> Result<ExecutedBuild, PackageCommandError> {
        let source = acquired.source_capture();
        let mut sources = PlanningSources::CapturedWithLibraries {
            source,
            libraries: self.supplied_libraries(acquired, supplied)?,
        };
        let mut path = entry.to_path_buf();
        let mut preparations = Vec::new();
        loop {
            let mut execution =
                self.session
                    .source_build_inputs(self.profile, snapshot.take(), dependency)?;
            if !dependency {
                execution.programs = self.root_programs.clone();
                let previously_accepted = crate::review::RootRestrictedBuildGrants::for_source(
                    &self.acquisitions.root_policies,
                    acquired,
                    self.profile,
                );
                let phase = self.acquisitions.phase_grants.clone();
                let grants = match (previously_accepted, phase) {
                    (Some(mut previous), phase) => {
                        previous.phase = phase;
                        Some(previous)
                    }
                    (None, phase) => phase.map(crate::review::RootRestrictedBuildGrants::for_phase),
                };
                if let Some(grants) = grants {
                    execution.programs.reviewed_root_name = grants.name.clone();
                    execution.restricted_grants = Some(Box::new(grants));
                }
            }
            let stage = project_build::prepare(
                PlanningRequest {
                    root: &path,
                    sources,
                    execution,
                    artifact_only: false,
                },
                Default::default(),
                planning_timings(self.timings),
            )
            .map_err(|failure| PackageCommandError::for_build_preparation(failure, !dependency))?;
            let mut requests = Default::default();
            let stage = project_build::interpret(
                stage,
                &mut services::SourceServices {
                    host: self,
                    acquired,
                    requests: &mut requests,
                },
            )?;
            self.session
                .record_source_completion(stage.source_completion_accounting().clone());
            let Some(next_entry) = stage.returned().successor_entry().cloned() else {
                return Ok(ExecutedBuild {
                    stage,
                    requests,
                    preparations,
                });
            };
            self.acquisitions.admit_successor()?;
            let (libraries, inputs) =
                self.successor_inputs(acquired, stage.returned(), requests)?;
            preparations.push((next_entry, inputs));
            let successor = stage.finish_successor(libraries)?;
            path = successor.path().to_owned();
            sources = PlanningSources::Successor(successor);
        }
    }
}
