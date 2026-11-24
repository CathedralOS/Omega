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
use evaluation_accounting::BuildEvaluationSponsor;
use evaluation_contracts::BuildEvaluationSponsorLimits;
use std::sync::Arc;

pub struct PreparedBuildExecution {
    earlier: Vec<terminal_codec::RetainedTerminalCode>,
    predecessor: Option<(
        terminal_codec::TerminalExecutionIdentity,
        Arc<source_compiler::publication::AcceptedTerminalSource>,
    )>,
    pub(crate) original_sources: Option<Arc<super::OriginalBuildSources>>,
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<source_compiler::publication::ProducedTerminalProgram>,
    accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: evaluation_accounting::EvaluationInvocationAccount,
    /// The sponsor the caller supplied, kept for the admission of a later
    /// stage of the same activation.
    evaluation_sponsor: Option<BuildEvaluationSponsor>,
    machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
    filesystem_scope: package_compilation::BuildMachineFilesystemScope,
    artifact_only: bool,
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
            artifact_only,
            default_source,
            default_sources,
        } = admitted;
        let refuse = |reason: String| failed(&machine.name, &reason);
        let providers = providers.admit(&produced)?;

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
            earlier: Vec::new(),
            predecessor: None,
            original_sources,
            execution_purpose,
            accepted: prepared,
            produced,
            providers: Arc::new(providers),
            account,
            evaluation_sponsor: supplied_sponsor,
            machine: Arc::new(machine),
            filesystem_scope,
            artifact_only,
            host_issued: false,
            default_source,
            default_sources,
            execution_profile,
        })
    }

    /// Admits the accepted program of the next stage of this activation
    /// (`source-to-psi/driver/compile_build/waits`) under the filesystem scope, sponsor and
    /// execution profile the activation already has.
    pub(crate) fn admit_stage(
        &self,
        prepared: source_compiler::CheckedBuildInvocation,
        providers: crate::admission::provider_execution::BuildProviderPlans,
        machine: symbols::SymbolHandle,
        source: package_compilation::BuildSourceSelection,
        package_inputs: Option<&package_compilation::PackageCompilationInputs>,
    ) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
        crate::admit_build_program(
            prepared,
            providers,
            machine,
            &self.filesystem_scope,
            self.evaluation_sponsor.as_ref(),
            self.execution_profile,
            self.artifact_only,
            source,
            package_inputs,
        )
    }

    /// The execution of the next stage. It is charged to the same invocation
    /// account as the stages before it: one Build activation is one
    /// invocation, however many programs it runs as.
    pub(crate) fn continue_with(
        &mut self,
        admitted: AdmittedBuildProgram,
        original_sources: Option<Arc<super::OriginalBuildSources>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let AdmittedBuildProgram {
            prepared,
            produced,
            providers,
            mut machine,
            filesystem_scope,
            evaluation_sponsor: _,
            execution_profile,
            artifact_only,
            default_source,
            default_sources,
        } = admitted;
        // Files an earlier stage staged are settled when the last one
        // returns, whether or not that stage reaches the filesystem itself.
        machine.filesystem_reachable |= self.machine.filesystem_reachable;
        let providers = providers.admit(&produced)?;
        let mut earlier = std::mem::take(&mut self.earlier);
        earlier.push(self.produced.retain_code());
        Ok(Self {
            earlier,
            predecessor: Some((
                self.produced.view().custody_identity(),
                Arc::clone(&self.accepted),
            )),
            original_sources,
            execution_purpose: self.execution_purpose,
            produced,
            providers: Arc::new(providers),
            accepted: prepared,
            account: self.account.clone(),
            evaluation_sponsor: self.evaluation_sponsor.clone(),
            machine: Arc::new(machine),
            filesystem_scope,
            artifact_only,
            host_issued: false,
            default_source,
            default_sources,
            execution_profile,
        })
    }

    /// Each path ends in an authenticated dependency preparation. The interpreter
    /// parks only on the one its ordinary control flow actually reaches.
    pub fn arm_preparations(
        &self,
        activation: &mut crate::TerminalBuildActivation,
        stage: &source_compiler::BuildStage,
    ) -> Result<(), Vec<Diagnostic>> {
        let paths = stage.outgoing_preparation_paths();
        if paths.is_empty()
            || activation.host.machine.preparation_allows_return()
                != Some(stage.can_finish_before_preparation())
        {
            return Err(
                self.refuse("the preparation paths disagree with the selected Build judgment")
            );
        }
        if activation.host.produced.view().custody_identity()
            != self.produced.view().custody_identity()
            || !self.account.same_invocation(&activation.host.account)
            || activation.cut.is_some()
        {
            return Err(self.refuse("the preparation cut belongs to another Build activation"));
        }
        let mut calls = Vec::with_capacity(paths.len());
        for path in paths {
            let sites = self.preparation_sites(path)?;
            let projected: Vec<_> = path.iter().map(|site| site.projects_suffix).collect();
            let call = activation
                .instance
                .select_projected_call_stack(&sites, &projected)
                .map_err(|error| self.refuse(&super::describe(error)))?;
            calls.push(call);
        }
        let calls = activation
            .instance
            .select_entry_calls(calls)
            .map_err(|error| self.refuse(&super::describe(error)))?;
        activation.cut = Some(super::ArmedPreparations {
            paths: paths.to_vec(),
            can_return: stage.can_finish_before_preparation(),
            calls,
        });
        Ok(())
    }

    fn preparation_sites(
        &self,
        path: &[source_compiler::BuildPreparationSite],
    ) -> Result<Vec<super::EntryCall>, Vec<Diagnostic>> {
        let sites = path
            .iter()
            .map(|site| {
                source_compiler::locate_build_suspending_call(
                    &self.accepted,
                    &site.owner,
                    site.call,
                )
                .map_err(|reason| self.refuse(reason))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let &(entry, operation) = sites
            .last()
            .ok_or_else(|| self.refuse("a Build preparation has no authored call"))?;
        let view = self.produced.view();
        let target = view
            .module()
            .machine(entry)
            .and_then(|machine| machine.operation(operation))
            .and_then(|op| op.declared_invocation())
            .ok_or_else(|| self.refuse("the authored preparation lost its admitted call"))?
            .callee;
        let calls = self
            .produced
            .build_requests()
            .calls_for(view)
            .ok_or_else(|| self.refuse("the preparation requests belong to another program"))?;
        if !calls.iter().any(|call| {
            call.action
                == source_compiler::publication::TerminalBuildRequestOperation::PrepareDependency
                && match target {
                    terminal_psi::TargetMachineCallable::Machine(wrapper) => {
                        // Every normal return has executed every operation in the entry block.
                        call.site.machine == wrapper
                            && view
                                .module()
                                .machine(wrapper)
                                .and_then(|machine| machine.block(&machine.entry))
                                .is_some_and(|block| {
                                    block
                                        .operations
                                        .iter()
                                        .any(|op| op.id == call.site.operation)
                                })
                    }
                    terminal_psi::TargetMachineCallable::Boundary(boundary) => {
                        call.site.machine == entry
                            && call.site.operation == operation
                            && call.site.boundary == boundary
                    }
                }
        }) {
            return Err(self.refuse(
                "the authored preparation does not invoke the admitted dependency protocol",
            ));
        }
        Ok(sites)
    }

    pub fn resume(
        &mut self,
        previous: crate::TerminalBuildActivation,
        path: &[source_compiler::BuildPreparationSite],
    ) -> Result<crate::TerminalBuildActivation, Vec<Diagnostic>> {
        if self.host_issued {
            return Err(self.refuse("Build invocation already issued its execution host"));
        }
        let (identity, predecessor) = self.predecessor.take().ok_or_else(|| {
            self.refuse("Build resume requires the preceding stage of this invocation")
        })?;
        self.host_issued = true;
        if !self.account.same_invocation(&previous.host.account)
            || identity != previous.host.produced.view().custody_identity()
        {
            return Err(self.refuse("Build resume requires the preceding stage of this invocation"));
        }
        let sites = self.preparation_sites(path)?;
        let &(machine, operation) = sites
            .last()
            .ok_or_else(|| self.refuse("missing successor preparation"))?;
        let mut carried = previous
            .carry(path, (machine, operation))
            .map_err(|reason| self.refuse(&reason))?;
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
            carried.filesystem.take(),
        )
        .and_then(|host| host.resume(carried, &predecessor))
        .map_err(|reason| {
            let mut diagnostics = self.refuse(&reason);
            if let Some(owner) = self.accepted.closure().elaboration().machines.get(machine)
                .and_then(|symbol| self.accepted.source().symbols.symbol_provenance_source_span(*symbol))
                && let Some(site) = path.last()
            {
                for diagnostic in &mut diagnostics {
                    diagnostic.source_span = Some(source_text::SourceSpan::new(owner.source_id, site.call));
                }
            }
            diagnostics
        })
    }

    pub fn host(&mut self) -> Result<Arc<crate::TerminalBuildHost>, Vec<Diagnostic>> {
        if self.predecessor.is_some() {
            return Err(self.refuse("a later Build stage must resume its preceding activation"));
        }
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
            None,
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
        mut self,
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
        self.earlier.push(self.produced.retain_code());
        Ok(ComputedBuildConfig {
            execution_history: history,
            accepted_builds: self.earlier,
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
/// made, described for its refusal. Provider selections and `Build::bind`
/// native bindings choose implementations for the executable product, which
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
