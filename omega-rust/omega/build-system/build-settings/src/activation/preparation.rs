/*
Prepare an `AdmittedBuildProgram` for Terminal execution and settle its
normal-return `ComputedBuildConfig` (`PreparedBuildExecution`).

`PreparedBuildExecution::prepare` takes the one invocation-scoped program whose
source selections were admitted by `admit_build_program`, admits its
`BuildProviderPlans` against that same produced artifact view, and opens a
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
use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};
use std::sync::Arc;

pub struct PreparedBuildExecution {
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<source_compiler::ProducedTerminalProgram>,
    accepted: Arc<source_compiler::AcceptedTerminalSource>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: evaluation_accounting::EvaluationInvocationAccount,
    /// The sponsor the caller supplied, kept for the admission of a later
    /// stage of the same activation.
    evaluation_sponsor: Option<BuildEvaluationSponsor>,
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
        let providers = providers.admit(produced.view())?;

        let measurement = crate::activation::measurement();
        const INVOCATION_WORK: u64 = 100_000;
        let supplied_sponsor = evaluation_sponsor.clone();
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
            execution_purpose,
            accepted: prepared,
            produced,
            providers: Arc::new(providers),
            account,
            evaluation_sponsor: supplied_sponsor,
            machine: Arc::new(machine),
            filesystem_scope,
            host_issued: false,
            default_source,
            default_sources,
            execution_profile,
        })
    }

    /// Admits the accepted program of the next stage of this activation
    /// (`admission/build_stages.rs`) under the filesystem scope, sponsor and
    /// execution profile the activation already has.
    pub(crate) fn admit_stage(
        &self,
        prepared: Arc<source_compiler::AcceptedTerminalSource>,
        providers: crate::admission::provider_execution::BuildProviderPlans,
        machine: symbols::SymbolHandle,
        source: package_compilation::BuildSourceSelection,
        package_inputs: Option<&package_compilation::PackageCompilationInputs>,
        anonymous_source: Option<&package_compilation::AnonymousSourceCapture>,
    ) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
        crate::admit_build_program(
            prepared,
            providers,
            machine,
            &self.filesystem_scope,
            self.evaluation_sponsor.as_ref(),
            self.execution_profile,
            false,
            source,
            package_inputs,
            anonymous_source,
        )
    }

    /// The execution of the next stage. It is charged to the same invocation
    /// account as the stages before it: one Build activation is one
    /// invocation, however many programs it runs as.
    pub(crate) fn continue_with(
        &self,
        admitted: AdmittedBuildProgram,
    ) -> Result<Self, Vec<Diagnostic>> {
        let AdmittedBuildProgram {
            prepared,
            produced,
            providers,
            mut machine,
            filesystem_scope,
            evaluation_sponsor: _,
            execution_profile,
            artifact_only: _,
            default_source,
            default_sources,
        } = admitted;
        // Files an earlier stage staged are settled when the last one
        // returns, whether or not that stage reaches the filesystem itself.
        machine.filesystem_reachable |= self.machine.filesystem_reachable;
        let providers = providers.admit(produced.view())?;
        Ok(Self {
            execution_purpose: self.execution_purpose,
            produced,
            providers: Arc::new(providers),
            // Package review reads the contracts of the accepted program
            // this execution holds: for a staged Build, its last stage.
            accepted: prepared,
            account: self.account.clone(),
            evaluation_sponsor: self.evaluation_sponsor.clone(),
            machine: Arc::new(machine),
            filesystem_scope,
            host_issued: false,
            default_source,
            default_sources,
            execution_profile,
        })
    }

    /// Starts this stage where `previous`, the stage before it, returned.
    pub fn resume(
        &mut self,
        previous: crate::TerminalBuildActivation,
    ) -> Result<crate::TerminalBuildActivation, Vec<Diagnostic>> {
        if self.host_issued {
            return Err(self.refuse("Build invocation already issued its execution host"));
        }
        self.host_issued = true;
        let mut carried = previous.carry().map_err(|reason| self.refuse(&reason))?;
        crate::TerminalBuildHost::new(
            self.execution_purpose,
            Arc::clone(&self.produced),
            Arc::clone(&self.providers),
            &self.account,
            Arc::clone(&self.machine),
            &self.filesystem_scope,
            self.execution_profile,
            self.default_source,
            self.default_sources.clone(),
            carried.filesystem.take(),
        )
        .and_then(|host| host.resume(carried))
        .map_err(|reason| self.refuse(&reason))
    }

    pub fn host(&mut self) -> Result<Arc<crate::TerminalBuildHost>, Vec<Diagnostic>> {
        if self.host_issued {
            return Err(self.refuse("Build invocation already issued its execution host"));
        }
        self.host_issued = true;
        crate::TerminalBuildHost::new(
            self.execution_purpose,
            Arc::clone(&self.produced),
            Arc::clone(&self.providers),
            &self.account,
            Arc::clone(&self.machine),
            &self.filesystem_scope,
            self.execution_profile,
            self.default_source,
            self.default_sources.clone(),
            None,
        )
        .map_err(|reason| self.refuse(&reason))
    }

    pub(crate) fn validate_return(&self, returned: &ReturnedBuild) -> Result<(), Vec<Diagnostic>> {
        if !self.host_issued || returned.identity != self.produced.view().custody_identity() {
            return Err(self.refuse("Build completion belongs to another invocation"));
        }
        Ok(())
    }

    pub(crate) fn complete(
        self,
        completed: crate::ReturnedBuild,
    ) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        self.validate_return(&completed)?;
        let declaration = &completed.declaration;
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
        let artifact_only = matches!(declaration, Some(build_declarations::BuildDeclaration::Application(application)) if application.artifact_only);
        if artifact_only && !completed.root_bindings.is_empty() {
            return Err(
                self.refuse("artifact-only Build cannot select executable entries or providers")
            );
        }
        let mut observations =
            package_compilation::CapturedBuildObservations::new(&self.filesystem_scope);
        if let Some(filesystem) = completed.filesystem {
            observations.retain_filesystem_effects(filesystem)?;
        }
        observations.retain_build_log(
            completed
                .log
                .publish(&self.account)
                .map_err(|reason| self.refuse(&reason))?,
        )?;
        let observation_summary =
            observations.settle(self.machine.filesystem_reachable, &self.machine.name)?;
        if artifact_only && observation_summary.required_output_settlements().is_empty() {
            return Err(self.refuse("artifact-only Build requires at least one completed output"));
        }
        let generated_sources = observation_summary.select_generated_sources(&self.machine.name)?;
        let usage = *self
            .account
            .snapshot()
            .map_err(|error| self.refuse(&error.to_string()))?
            .record();
        let native_binding_evaluation = if completed.native_bindings.is_empty() {
            None
        } else {
            Some(Arc::new(
                provider_planning::native_bindings::NativeBuildEvaluation::from_execution(
                    completed.identity.semantic(),
                    usage,
                    &observation_summary,
                )
                .map_err(|reason| self.refuse(&reason))?,
            ))
        };
        let config = BuildConfig {
            settings: completed.settings,
            native_binding_evaluation,
            ..Default::default()
        };
        Ok(ComputedBuildConfig {
            accepted_build: Some(self.accepted),
            config,
            root_binding_requests: completed.root_bindings,
            declared_product_selections: completed.declared_product_selections,
            product_selection_requests: completed.product_selections,
            native_binding_requests: completed.native_bindings,
            native_formations: completed.native_formations,
            behavior_exclusion_requests: completed.exclusions,
            project_declaration: completed.declaration,
            optimization_report_request: completed.optimization_report,
            evaluation_usage: Some(usage),
            observation_summary: Some(observation_summary),
            selected_build_machine_symbol: Some(self.machine.symbol),
            generated_sources,
        })
    }

    fn refuse(&self, reason: &str) -> Vec<Diagnostic> {
        failed(&self.machine.name, reason)
    }
}

fn failed(name: &str, reason: &str) -> Vec<Diagnostic> {
    let message = format!("Terminal Build `{name}` failed: {reason}");
    vec![Diagnostic::error(message)]
}
