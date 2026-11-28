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
        dependency: Option<(
            build_declarations::DependencyPurpose,
            &build_execution::DependencyBacking,
        )>,
        supplied: &[BuildLibraryRequest],
    ) -> Result<ExecutedBuild, PackageCommandError> {
        let subject = dependency
            .filter(|_| !self.staged)
            .map(|(purpose, _)| {
                crate::transaction::restricted_build::dependency_subject(acquired, purpose)
            })
            .transpose()?;
        let mut receiving = acquired.clone();
        let source = acquired.source_capture();
        let mut sources = PlanningSources::CapturedWithLibraries {
            source,
            libraries: self.supplied_libraries(acquired, supplied)?,
        };
        let mut path = entry.to_path_buf();
        let mut preparations = Vec::new();
        loop {
            let mut execution = self.session.source_build_inputs(
                self.profile,
                snapshot.take(),
                dependency.is_some(),
            )?;
            if let Some((_, backing)) = dependency {
                execution.network = backing.network.as_deref().copied().unwrap_or_default();
                execution.programs.allow = !backing.programs.is_empty();
                execution.programs.worker = self.root_programs.worker.clone();
                execution.programs.inherited = backing.programs.clone();
            } else {
                execution.programs = self.root_programs.clone();
                execution.network = self.root_network;
            }
            if self.staged {
                execution.restricted_grants = Some(Box::new(StagedGrants {
                    network: execution.network,
                    program_launch: execution.programs.allow,
                }));
            } else {
                let previously_accepted = crate::review::OccurrenceBuildGrants::for_source(
                    &self.acquisitions.root_policies,
                    acquired,
                    self.profile,
                    dependency.map(|(purpose, _)| purpose),
                );
                let phase = self.acquisitions.phase_grants.clone();
                let grants = match (previously_accepted, phase) {
                    (Some(mut previous), phase) => {
                        previous.phase = phase;
                        Some(previous)
                    }
                    (None, phase) => phase.map(crate::review::OccurrenceBuildGrants::for_phase),
                };
                if let Some(mut grants) = grants {
                    grants.subject = subject
                        .clone()
                        .unwrap_or(package_compilation::RestrictedBuildSubject::Root);
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
                self.preparation.clone(),
                planning_timings(self.timings),
            )?;
            let mut requests = Default::default();
            let stage = project_build::interpret(
                stage,
                &mut services::SourceServices {
                    host: self,
                    acquired: &receiving,
                    requests: &mut requests,
                },
            )?;
            self.session
                .record_source_completion(stage.source_completion_accounting().clone());
            let Some(next_entry) = stage.returned().successor_entry().cloned() else {
                return Ok(ExecutedBuild {
                    receiving,
                    stage,
                    requests,
                    preparations,
                });
            };
            self.acquisitions.admit_successor()?;
            let (libraries, inputs) =
                self.successor_inputs(&receiving, stage.returned(), requests)?;
            preparations.push((next_entry, inputs));
            let successor = stage.finish_successor(libraries)?;
            receiving.follow_successor(&successor)?;
            path = successor.path().to_owned();
            sources = PlanningSources::Successor(successor);
        }
    }
}

/// The invocation or the parent registration supplied these resources to this
/// occurrence. Provider admission has already matched every executable binding
/// and identified the exact peer of each reachable live datagram request.
struct StagedGrants {
    network: package_compilation::build_execution::BuildNetworkInvocation,
    program_launch: bool,
}

impl package_compilation::build_execution::RestrictedBuildGrants for StagedGrants {
    fn admits(
        &mut self,
        _site: &package_compilation::build_execution::RestrictedBuildSite,
        request: &package_compilation::build_execution::RestrictedBuildRequest,
    ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
        use package_compilation::build_execution::{
            BuildNetworkInvocation, RestrictedBuildOperation,
        };
        Ok(match request.operation() {
            RestrictedBuildOperation::SendDatagram { peer }
            | RestrictedBuildOperation::ReceiveDatagram { peer } => {
                self.network == (BuildNetworkInvocation::Granted { peer: Some(*peer) })
            }
            RestrictedBuildOperation::ProgramLaunch(_) => self.program_launch,
            // Explicit directory mappings already have per-path custody in the
            // captured filesystem. These rows request a separate real filesystem.
            RestrictedBuildOperation::ScopedFilesystemExecution
            | RestrictedBuildOperation::UnscopedFilesystemExecution => false,
        })
    }
}
