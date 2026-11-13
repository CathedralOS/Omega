/*
Bind the admitted `Build` entry, operational reach, and host filesystem scope
into an activation-local `AdmittedBuildProgram` checkpoint before any authored
code runs (`admit_build_program`).

`AdmittedBuildProgram` is the sole capability token accepted by
`PreparedBuildExecution::prepare`:
- `restricted_build_requests` projects the normalized `RestrictedBuildRequest`
  list in issue order (using package vocabulary rather than host paths or live
  handles) so `prepare_build_evaluation` can enforce caller consent before
  execution.
- `pure_consumption_inputs` constructs the immutable `CompletedPureBuildInputs`
  lookup key only when the `Build` machine is `Pure`, has no reachable
  filesystem operations, and projects no restricted host requests.
*/

use build_time_evaluation::{BuildEvaluationSponsor, PreparedBuildMachineProgram};
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use symbols::SymbolHandle;

#[must_use = "an admitted build program must prepare its execution"]
pub(crate) struct AdmittedBuildProgram {
    pub(crate) prepared: PreparedBuildMachineProgram,
    pub(crate) providers: super::provider_execution::BuildProviderPlans,
    pub(crate) machine: SelectedBuildMachine,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    pub(crate) execution_profile: Option<target::TargetProfile>,
    pub(crate) artifact_only: bool, // Validated `Build::artifact_only(builder)` modifier: publishes retained outputs only, forbids executable roots/providers, and requires at least one required output.
    pub(crate) default_source: bool,
    pub(crate) default_sources:
        Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
}

impl AdmittedBuildProgram {
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        self.filesystem_scope.restricted_build_requests(
            self.evaluation_sponsor.as_ref(),
            self.artifact_only,
            &self.machine.execution_mode,
        )
    }

    pub(crate) fn pure_consumption_inputs(
        &self,
        packages: Option<&package_compilation::PackageCompilationInputs>,
        snapshot: Option<&package_compilation::BuildSnapshotRequest>,
    ) -> Option<crate::activation::completed_pure_build::CompletedPureBuildInputs> {
        if self.machine.filesystem_reachable
            || self.machine.execution_mode != build_time_evaluation::BuildMachineExecutionMode::Pure
            || !self.restricted_build_requests().is_empty()
        {
            return None;
        }
        let checked = self.prepared.checked_program()?;
        crate::activation::completed_pure_build::CompletedPureBuildInputs::from_admitted(
            checked,
            self.providers.clone(),
            self.machine.symbol,
            self.machine.initial_build.clone(),
            packages,
            snapshot,
            &self.filesystem_scope,
            self.artifact_only,
            self.default_source,
            crate::activation::measurement(),
            self.evaluation_sponsor.as_ref(),
        )
    }
}

pub(crate) fn admit_build_program(
    prepared: PreparedBuildMachineProgram,
    providers: super::provider_execution::BuildProviderPlans,
    machine_symbol: SymbolHandle,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    execution_profile: Option<target::TargetProfile>,
    artifact_only: bool,
    source: package_compilation::BuildSourceSelection,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let typed = prepared.typed();
    if !typed
        .symbols
        .symbol_source_span(machine_symbol)
        .is_some_and(|span| span.source_id == source.source_id())
    {
        return Err(vec![Diagnostic::error(
            "Build entry differs from its source-owned selection",
        )]);
    }
    let mut filesystem_scope = filesystem_scope.clone();
    let machine = admit_build_machine(&prepared, machine_symbol, &mut filesystem_scope)?;
    Ok(AdmittedBuildProgram {
        prepared,
        providers,
        machine,
        filesystem_scope,
        evaluation_sponsor: evaluation_sponsor.cloned(),
        execution_profile,
        artifact_only,
        default_source: source.is_default(),
        default_sources: package_inputs
            .filter(|_| source.is_default())
            .map(package_compilation::PackageCompilationInputs::source_inputs),
    })
}
