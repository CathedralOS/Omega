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
        let mut libraries = self.supplied_libraries(acquired, supplied)?;
        let mut path = entry.to_path_buf();
        let mut preparations = project_build::BuildPreparations::default();
        let mut visited = Vec::new();
        loop {
            let sources = if preparations.is_empty() {
                PlanningSources::CapturedWithLibraries { source, libraries }
            } else {
                // Each successor here sees the same immutable tree and a fresh Build.
                // Repeating its file and exact incoming libraries would repeat the execution.
                if visited
                    .iter()
                    .any(|(file, inputs)| file == &path && inputs == &libraries)
                {
                    return Err(format!(
                        "Build successor repeats the same inputs for {}",
                        path.display()
                    )
                    .into());
                }
                visited.push((path.clone(), libraries.clone()));
                PlanningSources::Successor {
                    source,
                    previous_build_files: preparations.build_files(),
                    libraries,
                }
            };
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
            let Some(next) = stage.returned().successor_entry() else {
                return Ok(ExecutedBuild {
                    stage: stage.with_preparations(preparations),
                    requests,
                });
            };
            let next_path = source.canonical_live_root().join(next.as_str());
            self.acquisitions.admit_successor()?;
            libraries = self.successor_libraries(acquired, stage.returned(), requests)?;
            stage.finish_successor(&mut preparations, source, &libraries)?;
            path = next_path;
        }
    }
}
