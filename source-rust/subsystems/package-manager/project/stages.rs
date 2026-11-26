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
        loop {
            let execution =
                self.session
                    .source_build_inputs(self.profile, snapshot.take(), dependency)?;
            let stage = project_build::prepare(
                PlanningRequest {
                    root: &path,
                    sources,
                    execution,
                    artifact_only: false,
                },
                Default::default(),
                planning_timings(self.timings),
            )?;
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
            if stage.returned().successor_entry().is_none() {
                return Ok(ExecutedBuild { stage, requests });
            }
            self.acquisitions.admit_successor()?;
            let libraries = self.successor_libraries(acquired, stage.returned(), requests)?;
            let successor = stage.finish_successor(source, libraries)?;
            path = successor.path().to_owned();
            sources = PlanningSources::Successor(successor);
        }
    }
}
