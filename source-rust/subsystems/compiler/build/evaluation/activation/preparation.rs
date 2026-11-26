/*
Prepare an `AdmittedBuildProgram` for Terminal execution and settle its
normal-return `ComputedBuildConfig` (`PreparedBuildExecution`).

`PreparedBuildExecution::prepare` takes the one invocation-scoped program whose
source selections were admitted by `admit_build_program`, admits its
`BuildProviderScope` against that same produced artifact view, and opens a
`100,000`-work `EvaluationInvocationAccount` under the caller's
`BuildEvaluationSponsor` (or a standalone per-invocation sponsor with Terminal
measurement identity if none was supplied).

`host` issues a single `TerminalBuildHost` (`host_issued` guard) so a dropped or
failed activation cannot be restarted, and `complete` verifies that `ReturnedBuild`
carries this exact invocation's `custody_identity`, confirms that any issued
dependency requests were joined by the coordinator (`source_join`), settles
filesystem observations and build logs, and assembles `ComputedBuildConfig`.
*/

use crate::{AdmittedBuildProgram, BuildConfig, ComputedBuildConfig, ReturnedBuild};
use build_declarations::BuildRequest;
use diagnostics::Diagnostic;
use evaluation_accounting::BuildEvaluationSponsor;
use evaluation_contracts::BuildEvaluationSponsorLimits;
use std::sync::Arc;

pub struct PreparedBuildExecution {
    original_sources: Option<Arc<super::OriginalBuildSources>>,
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<source_compiler::publication::ProducedTerminalProgram>,
    accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: evaluation_accounting::EvaluationInvocationAccount,
    machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
    filesystem_scope: package_compilation::BuildMachineFilesystemScope,
    host_issued: bool,
    default_source: bool,
    default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    execution_profile: Option<target::TargetProfile>,
}

impl PreparedBuildExecution {
    pub(crate) fn prepare(
        admitted: AdmittedBuildProgram,
        execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
        original_sources: Option<Arc<super::OriginalBuildSources>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let AdmittedBuildProgram {
            prepared,
            produced,
            providers,
            machine,
            filesystem_scope,
            evaluation_sponsor,
            execution_profile,
            artifact_only: _,
            default_source,
            default_sources,
        } = admitted;
        let refuse = |reason: String| failed(&machine.name, &reason);
        let providers = providers.admit(&produced).map_err(|mut errors| {
            if let Some(span) = prepared.source().symbols.symbol_source_span(machine.symbol) {
                for error in &mut errors {
                    error.source_span.get_or_insert(span);
                }
            }
            errors
        })?;

        let measurement = crate::activation::measurement();
        const INVOCATION_WORK: u64 = 100_000;
        let sponsor = match evaluation_sponsor {
            Some(sponsor) => sponsor,
            None => {
                let limits = BuildEvaluationSponsorLimits::new(
                    INVOCATION_WORK,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                )
                .ok_or_else(|| refuse("invalid standalone Build limits".into()))?;
                BuildEvaluationSponsor::new_with_measurement(limits, measurement)
                    .map_err(|error| refuse(error.to_string()))?
            }
        };
        let account = sponsor
            .begin_invocation(measurement, INVOCATION_WORK)
            .map_err(|error| refuse(error.to_string()))?;
        Ok(Self {
            original_sources,
            execution_purpose,
            accepted: prepared,
            produced,
            providers: Arc::new(providers),
            account,
            machine: Arc::new(machine),
            filesystem_scope,
            host_issued: false,
            default_source,
            default_sources,
            execution_profile,
        })
    }

    pub fn host(&mut self) -> Result<Arc<crate::TerminalBuildHost>, Vec<Diagnostic>> {
        if self.host_issued {
            return Err(self.refuse("Build invocation already issued its execution host"));
        }
        self.host_issued = true;
        crate::TerminalBuildHost::new(
            self.execution_purpose,
            Arc::clone(&self.produced),
            Arc::clone(&self.accepted),
            Arc::clone(&self.providers),
            &self.account,
            Arc::clone(&self.machine),
            &self.filesystem_scope,
            self.execution_profile,
            self.default_source,
            self.default_sources.clone(),
            self.original_sources.clone(),
        )
        .map_err(|reason| self.refuse(&reason))
    }

    pub(crate) fn validate_return(&self, returned: &ReturnedBuild) -> Result<(), Vec<Diagnostic>> {
        if !self.host_issued
            || !self.account.same_invocation(&returned.account)
            || returned.identity != self.produced.view().custody_identity()
        {
            return Err(self.refuse("Build completion belongs to another invocation"));
        }
        Ok(())
    }

    pub(crate) fn complete(
        self,
        completed: crate::ReturnedBuild,
    ) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        self.validate_return(&completed)?;
        if completed.successor_entry().is_some() {
            return Err(self.refuse("an interpreted successor must run before opening a Product"));
        }
        if completed.source_join.is_none()
            && completed
                .requests
                .iter()
                .any(|issued| matches!(issued.request(), BuildRequest::Dependency { .. }))
        {
            return Err(self.refuse(
                "dependency requests require coordinator settlement before Product compilation",
            ));
        }
        self.freeze(completed)
    }

    pub(crate) fn complete_successor(
        self,
        completed: ReturnedBuild,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    ) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        self.validate_return(&completed)?;
        if completed.successor_entry().is_none() {
            return Err(self.refuse("only a Build that selected a successor can prepare its entry"));
        }
        completed
            .validate_successor_libraries(libraries)
            .map_err(|reason| self.refuse(&reason))?;
        if !completed.root_bindings.is_empty()
            || completed.product_selections.iter().any(|event| event.request.operation != source_compiler::representations::declaration_selection::BuildOperation::ProviderSelection || event.test_group.is_some())
            || !completed.native_bindings.is_empty()
            || !completed.exclusions.is_empty()
            || completed.settings != Default::default()
            || completed.optimization_report != Default::default()
        {
            return Err(self.refuse(
                "this interpreted successor has an unsupported executable selection or setting",
            ));
        }
        self.freeze(completed)
    }

    fn freeze(self, completed: ReturnedBuild) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        let declaration = completed.declaration().cloned();
        let artifact_only = matches!(&declaration, Some(build_declarations::BuildDeclaration::Application(application)) if application.artifact_only);
        // Artifact-only mode builds no product, so an entry, provider or
        // native binding would have nothing to apply to. The spec
        // (scoped_execution.md, "Staged products and failure") requires a
        // refusal rather than silently dropping the selection.
        if artifact_only && let Some(reason) = executable_selection_in_artifact_only(&completed) {
            return Err(self.refuse(reason));
        }
        let mut observations =
            package_compilation::CapturedBuildObservations::new(&self.filesystem_scope);
        if let Some(filesystem) = completed.filesystem {
            observations.retain_filesystem_effects(filesystem)?;
        }
        observations.retain_source_publications(completed.output.sources, &self.account)?;
        observations.retain_build_log(
            completed
                .output
                .log
                .publish(&self.account)
                .map_err(|reason| self.refuse(&reason))?,
        )?;
        let observation_summary =
            observations.settle(self.machine.filesystem_reachable, &self.machine.name)?;
        if artifact_only && observation_summary.required_output_settlements().is_empty() {
            return Err(self.refuse("artifact-only Build requires at least one completed output"));
        }
        let generated_sources = observation_summary.generated_sources().to_vec();
        let usage = *self
            .account
            .snapshot()
            .map_err(|error| self.refuse(&error.to_string()))?
            .record();
        let history = Arc::new(completed.history);
        let native_binding_evaluation = if completed.native_bindings.is_empty() {
            None
        } else {
            Some(Arc::new(
                evaluation_receipts::NativeBuildEvaluation::from_execution(
                    completed.identity.semantic(),
                    Arc::clone(&history),
                    usage,
                    observation_summary.identity().digest(),
                )
                .map_err(|reason| self.refuse(&reason))?,
            ))
        };
        let config = BuildConfig {
            settings: completed.settings,
            native_binding_evaluation,
            ..Default::default()
        };
        let activation = source_compiler::representations::source_symbols::retain_declaration(
            &self.accepted.source().symbols,
            self.machine.symbol,
        )
        .map_err(|reason| self.refuse(reason))?;
        let selected_build_declaration =
            source_admission::RetainedDeclaration::from_retained_parts(
                activation.source().clone(),
                activation.span(),
                activation.kind(),
            )
            .map_err(|reason| self.refuse(reason))?;
        Ok(ComputedBuildConfig {
            execution_history: history,
            accepted_builds: vec![self.produced.retain_code()],
            config,
            root_binding_requests: completed.root_bindings,
            declared_product_selections: completed.declared_product_selections,
            product_selection_requests: completed.product_selections,
            native_binding_requests: completed.native_bindings,
            native_formations: completed.native_formations,
            behavior_exclusion_requests: completed.exclusions,
            project_declaration: declaration,
            optimization_report_request: completed.optimization_report,
            evaluation_usage: Some(usage),
            observation_summary: Some(observation_summary),
            executed_entry: Some(build_evaluation_result::ExecutedBuildEntry {
                declaration: selected_build_declaration,
                callable_identity: self.machine.normalized_callable_identity.clone(),
            }),
            generated_sources,
        })
    }

    fn refuse(&self, reason: &str) -> Vec<Diagnostic> {
        failed(&self.machine.name, reason)
    }
}

/// The first executable root or provider selection an artifact-only Build
/// made, described for its refusal. Provider selections and `Build::bind_native`
/// choose implementations for the executable product, which
/// artifact-only mode never builds.
fn executable_selection_in_artifact_only(completed: &crate::ReturnedBuild) -> Option<&'static str> {
    use source_compiler::representations::declaration_selection::BuildOperation;
    if !completed.root_bindings.is_empty() {
        return Some("artifact-only Build cannot bind executable entries");
    }
    if completed
        .product_selections
        .iter()
        .any(|selection| selection.request.operation == BuildOperation::ProviderSelection)
    {
        return Some("artifact-only Build cannot select providers");
    }
    if !completed.native_bindings.is_empty()
        || completed
            .product_selections
            .iter()
            .any(|selection| selection.request.operation == BuildOperation::NativeBinding)
    {
        return Some("artifact-only Build cannot bind native implementations");
    }
    None
}

fn failed(name: &str, reason: &str) -> Vec<Diagnostic> {
    let message = format!("Terminal Build `{name}` failed: {reason}");
    vec![Diagnostic::error(message)]
}
