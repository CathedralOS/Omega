/*
Retain and step a single source-issued `Build` activation (`TerminalBuildHost`,
`TerminalBuildActivation`, `ReturnedBuild`).

Why `TerminalBuildActivation` is a stateful coroutine owned by the caller rather
than a callback loop:
1. One `TerminalBuildHost` mints a single invocation-local root receiver
   (`original = TerminalStructuralValue::minted(1, ...)`), wires the request,
   binding, and filesystem host adapters (`TerminalBuildRequestHost`,
   `TerminalBuildBindingHost`, `TerminalBuildFilesystemHost`) to that exact
   receiver token, and starts at most one `TerminalBuildActivation` (`started`
   atomic guard). Neither a second instance nor a guest-constructed struct of
   the same type can forge host requests.
2. `orchestration` drives the activation in work slices via `advance(additional_work)`.
   Whenever `advance` pauses on a provider readiness demand (`AwaitingEffect`) or
   a Product declaration query (`HostComputationPending`), the activation stays
   suspended in place with its live `TerminalInstance` and `EvaluationInvocationAccount`
   intact while `orchestration` compiles the required dependency and resumes via
   `install_provider` or `answer_query`.
3. Every mutating transition (`advance`, `install_provider`, `answer_query`) sets
   `self.state = ActivationState::Refused` before performing fallible work and
   restores `Running` or `Returned` only on success, so any partial failure
   permanently poisons the activation rather than letting later steps conceal an
   incomplete provider installation or failed query.
4. On normal return (`complete`), we extract the plain settings from borrowed
   entry `0` (`settings::extract`), close the interpreter instance before
   draining host selections and logs, validate the authored project role
   (`project::declaration`), and return `ReturnedBuild` with its complete
   request and demanded-source ledgers attached for coordinator source-join
   settlement.
*/

pub(crate) mod completed_pure_build;
pub(crate) mod preparation;
mod project;
mod provider_installation;
pub mod scalar_completion;
mod source_join;
mod source_result;
pub use source_join::BuildDependencySelection;
pub use source_result::BuildSourceResult;

use evaluation_accounting::EvaluationInvocationAccount;
use language_semantics::declaration_selection::{
    ExecutedBuildSelection, ExecutedEnumExclusion, ExecutedNativeBinding, ExecutedProductSelection,
    ExecutedRootBinding, NativeBindingFormation, ProductDeclarationRequest,
};
use package_compilation::build_execution::build_value::{
    BuildValueAdmission, EvaluatedBuildSettings,
};
use psi_compiler::ProducedTerminalProgram;
use psi_work::WorkMeter;
use std::sync::Arc;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstance, TerminalInstantiation,
    TerminalProgram, TerminalStructuralInputs, TerminalStructuralValue,
};

use crate::host::bindings::TerminalBuildBindingHost;
use crate::host::requests::{IssuedBuildRequest, TerminalBuildRequestHost};

pub(crate) fn measurement() -> evaluation_accounting::EvaluationMeasurementIdentity {
    evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
    }
}

pub struct ReturnedBuild {
    pub(crate) identity: terminal_codec::TerminalExecutionIdentity,
    pub(crate) declaration: Option<build_declarations::BuildDeclaration>,
    pub(crate) source_join: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    pub(crate) requests: Vec<IssuedBuildRequest>,
    pub(crate) demanded_sources: Vec<BuildDependencySelection>,
    pub(crate) settings: EvaluatedBuildSettings,
    pub(crate) optimization_report: optimization_core::OptimizationReportRequest,
    pub(crate) root_bindings: Vec<ExecutedRootBinding>,
    pub(crate) product_selections: Vec<ExecutedProductSelection>,
    pub(crate) native_formations: Vec<NativeBindingFormation>,
    pub(crate) native_bindings: Vec<ExecutedNativeBinding>,
    pub(crate) exclusions: Vec<ExecutedEnumExclusion>,
    pub(crate) declared_product_selections: Vec<Arc<ProductDeclarationRequest>>,
    pub(crate) log: crate::host::requests::BuildLog,
    pub(crate) filesystem: Option<package_compilation::build_execution::BuildFilesystemEffects>,
}

impl ReturnedBuild {
    pub fn source_result(&mut self) -> BuildSourceResult<'_> {
        BuildSourceResult::executed(self)
    }

    pub fn declaration(&self) -> Option<&build_declarations::BuildDeclaration> {
        self.declaration.as_ref()
    }

    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    pub fn demanded_sources(&self) -> &[BuildDependencySelection] {
        &self.demanded_sources
    }
}

pub struct TerminalBuildHost {
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<ProducedTerminalProgram>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: EvaluationInvocationAccount,
    original: TerminalStructuralValue,
    requests: Arc<TerminalBuildRequestHost>,
    bindings: Arc<TerminalBuildBindingHost>,
    filesystem: Arc<crate::host::filesystem::TerminalBuildFilesystemHost>,
    started: std::sync::atomic::AtomicBool,
    execution_profile: Option<target::TargetProfile>,
    value_admission: BuildValueAdmission,
    default_source: bool,
    default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
}

impl TerminalBuildHost {
    pub(crate) fn new(
        execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
        produced: Arc<ProducedTerminalProgram>,
        providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
        account: &EvaluationInvocationAccount,
        machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
        filesystem_scope: &package_compilation::BuildMachineFilesystemScope,
        execution_profile: Option<target::TargetProfile>,
        default_source: bool,
        default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    ) -> Result<Arc<Self>, String> {
        let artifact = produced.view();
        let parameter = machine.receiver_for(artifact.module())?;
        let original = TerminalStructuralValue::minted(1, parameter.structural_type, Vec::new());
        let requests = TerminalBuildRequestHost::new(
            artifact,
            produced.build_requests(),
            original.clone(),
            Some(account),
        )?;
        let bindings = TerminalBuildBindingHost::new(
            artifact,
            produced.build_bindings(),
            original.clone(),
            Some(account),
        )?;
        let filesystem = crate::host::filesystem::TerminalBuildFilesystemHost::new(
            &produced,
            &original,
            account,
            filesystem_scope,
            &machine.name,
        )?;
        Ok(Arc::new(Self {
            execution_purpose,
            produced,
            providers,
            account: account.clone(),
            original,
            requests,
            bindings,
            filesystem,
            started: std::sync::atomic::AtomicBool::new(false),
            execution_profile,
            value_admission: machine.value_admission,
            default_source,
            default_sources,
            machine,
        }))
    }

    pub fn start(self: &Arc<Self>) -> Result<TerminalBuildActivation, String> {
        if self.started.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err("Build host has already started its activation".into());
        }
        artifacts::compile_timings::counters::record_build_execution(self.execution_purpose);
        let artifact = self.produced.view();
        let program = TerminalProgram::retain(self.produced.retain_code());
        let request_calls = self
            .produced
            .build_requests()
            .calls_for(artifact)
            .ok_or("Build requests lost their artifact custody")?;
        let binding_calls = self
            .produced
            .build_bindings()
            .calls_for(artifact)
            .ok_or("Build bindings lost their artifact custody")?;
        let query_calls = self
            .produced
            .build_bindings()
            .queries_for(artifact)
            .ok_or("Build queries lost their artifact custody")?;
        let filesystem_calls = self
            .produced
            .build_filesystem()
            .calls_for(artifact)
            .ok_or("Build filesystem calls lost their artifact custody")?;
        let mut provider_slots = Vec::new();
        for call in binding_calls.iter() {
            if let language_semantics::declaration_selection::BuildSelectionRequest::ProviderReadiness(request) = &call.request {
                provider_slots.extend_from_slice(&request.requirement.boundaries);
            }
        }
        provider_slots.sort_unstable();
        provider_slots.dedup();
        let mut host_imports = request_calls
            .iter()
            .map(|call| call.site.boundary)
            .chain(binding_calls.iter().map(|call| call.site.boundary))
            .chain(query_calls.iter().map(|call| call.site.boundary))
            .chain(filesystem_calls.iter().map(|call| call.site.boundary))
            .collect::<Vec<_>>();
        host_imports.extend(
            self.providers
                .intrinsics
                .operations()
                .iter()
                .map(|(slot, _)| *slot),
        );
        host_imports.sort_unstable();
        host_imports.dedup();
        let mut supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                host_imports: &host_imports,
                installation: Some(&self.providers.checked),
                entry: None,
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .map_err(describe)?;
        for boundary in &provider_slots {
            supplies = program
                .defer_component(supplies, *boundary)
                .map_err(describe)?;
        }
        let mut instance = program
            .instantiate(TerminalInstantiation {
                structural_inputs: TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&self.original),
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                installation: Some(&supplies),
                account: Some(&self.account),
                ..Default::default()
            })
            .map_err(describe)?;
        terminal_runtime::bind_admitted_intrinsics(&mut instance, &self.providers.intrinsics)
            .map_err(|reason| format!("Build selected intrinsic binding: {reason:?}"))?;
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            if provider_slots.contains(&boundary) {
                continue;
            }
            let declaration = artifact
                .module()
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == boundary)
                .ok_or("Build import has no Terminal declaration")?;
            if request_calls
                .iter()
                .any(|call| call.site.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.requests.adapter(declaration.into()))
                    .map_err(describe)?;
            } else if binding_calls
                .iter()
                .any(|call| call.site.boundary == boundary)
                || query_calls
                    .iter()
                    .any(|call| call.site.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.bindings.adapter(declaration.into()))
                    .map_err(describe)?;
            } else if filesystem_calls
                .iter()
                .any(|call| call.site.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.filesystem.adapter(declaration.into()))
                    .map_err(describe)?;
            } else {
                return Err(format!(
                    "Build import {boundary:?} has no admitted host operation or installed provider"
                ));
            }
        }

        let work = WorkMeter::with_invocation(self.account.clone(), Some(0)).map_err(describe)?;
        Ok(TerminalBuildActivation {
            instance,
            host: Arc::clone(self),
            work,
            state: ActivationState::Running,
            requests: Vec::new(),
            demanded_sources: Vec::new(),
            helper_accounting: Default::default(),
        })
    }
}

enum ActivationState {
    Running,
    Returned,
    Refused,
}

pub struct TerminalBuildActivation {
    instance: TerminalInstance<'static, 'static>,
    host: Arc<TerminalBuildHost>,
    work: WorkMeter,
    state: ActivationState,
    requests: Vec<IssuedBuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
    helper_accounting: build_time_evaluation::SourceCompletionAccounting,
}

pub struct TerminalBuildStep<'step> {
    pub requests: &'step [IssuedBuildRequest],
    pub returned: bool,
    pub awaiting_provider: Option<crate::host::bindings::BuildProviderRequest>,
    pub awaiting_query: Option<Arc<crate::host::bindings::BuildProductQuery>>,
}

impl TerminalBuildActivation {
    pub fn helper_evaluation_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.helper_accounting
    }

    pub fn answer_query(
        &mut self,
        request: &crate::host::bindings::BuildProductQuery,
        answer: crate::host::bindings::BuildProductDescription,
        dependencies: &[BuildDependencySelection],
    ) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Product query answer requires a live activation".into());
        }
        self.state = ActivationState::Refused;
        for selected in dependencies {
            if selected.purpose != build_declarations::DependencyPurpose::Product
                || self
                    .requests
                    .iter()
                    .filter(|issued| {
                        matches!(
                            issued.request(),
                            build_declarations::BuildRequest::Dependency {
                                purpose: build_declarations::DependencyPurpose::Product,
                                ..
                            }
                        )
                    })
                    .nth(selected.request_ordinal)
                    .is_none()
            {
                return Err("Product query dependency has no issued Product request".into());
            }
        }
        self.host.bindings.answer_query(request, answer)?;
        self.demanded_sources.extend_from_slice(dependencies);
        self.state = ActivationState::Running;
        Ok(())
    }

    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    pub fn view(&self) -> terminal_codec::TerminalExecutionView<'_> {
        self.host.produced.view()
    }

    pub fn install_provider(
        &mut self,
        selected: BuildDependencySelection,
        provider: &psi_compiler::ProducedBuildProvider,
    ) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Build provider installation requires a live readiness pause".into());
        }
        self.state = ActivationState::Refused;
        let request = self
            .host
            .bindings
            .awaiting_provider()?
            .ok_or("Build has no provider readiness pause")?;
        let (alias, _) = request
            .backing
            .path
            .split_once("::")
            .ok_or("Build backing has no dependency qualifier")?;
        let issued = self
            .requests
            .iter()
            .filter_map(|issued| match issued.request() {
                build_declarations::BuildRequest::Dependency {
                    purpose: build_declarations::DependencyPurpose::Build,
                    alias,
                    ..
                } => Some(alias),
                _ => None,
            })
            .nth(selected.request_ordinal)
            .ok_or("Build provider has no issued Build dependency request")?;
        if selected.purpose != build_declarations::DependencyPurpose::Build
            || selected.package != provider.package()
            || issued
                .as_ref()
                .is_some_and(|issued| issued.as_str() != alias)
        {
            return Err(
                "Build provider differs from its exact acquired dependency selection".into(),
            );
        }
        provider_installation::admit_request(self.view(), &request, provider)?;
        if !self.host.bindings.has_installed_backing(provider)? {
            let profile = self
                .host
                .execution_profile
                .ok_or("Build provider installation has no admitted execution profile")?;
            let bindings = provider_installation::prepare(
                self.host.produced.view(),
                &request,
                provider,
                profile,
                &self.host.account,
                &mut self.helper_accounting,
            )?;
            for binding in bindings {
                self.instance.bind_component(binding)?;
            }
        }
        self.host.bindings.provider_installed(provider)?;
        self.demanded_sources.push(selected);
        self.state = ActivationState::Running;
        Ok(())
    }

    pub fn advance(&mut self, additional_work: u64) -> Result<TerminalBuildStep<'_>, String> {
        match self.state {
            ActivationState::Returned => return Err("Build activation has already returned".into()),
            ActivationState::Refused => return Err("Build activation has already failed".into()),
            ActivationState::Running => {}
        }
        self.state = ActivationState::Refused;
        self.work.replenish(additional_work).map_err(describe)?;
        let status = self
            .instance
            .invoke(&mut self.work)
            .map_err(|error| error.to_string())?;
        let first_request = self.requests.len();
        self.requests.extend(self.host.requests.drain_requests()?);
        let returned = match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => true,
            TerminalExecutionStatus::SponsorExhausted(_) => false,
            TerminalExecutionStatus::AwaitingEffect(_) => {
                if self.host.bindings.awaiting_provider()?.is_none() {
                    return Err("Build paused without an admitted provider demand".into());
                }
                false
            }
            TerminalExecutionStatus::HostComputationPending(_) => {
                if self.host.bindings.awaiting_query()?.is_none() {
                    return Err("Build paused without an admitted Product query".into());
                }
                false
            }
            other => return Err(format!("Build did not return normally: {other:?}")),
        };
        self.state = if returned {
            ActivationState::Returned
        } else {
            ActivationState::Running
        };
        Ok(TerminalBuildStep {
            requests: &self.requests[first_request..],
            returned,
            awaiting_provider: self.host.bindings.awaiting_provider()?,
            awaiting_query: self.host.bindings.awaiting_query()?,
        })
    }

    pub fn complete(mut self) -> Result<ReturnedBuild, String> {
        if !matches!(self.state, ActivationState::Returned) {
            return Err("Build completion requires its normal interpreter return".into());
        }
        let (settings, optimization_report) = crate::settings::extract(
            &self.instance,
            self.host.value_admission,
            &self.host.machine.initial_build,
        )?;
        self.instance.close().map_err(describe)?;
        drop(self.instance);
        let declaration = if self.host.default_source {
            if !self.requests.is_empty() {
                return Err("default Build unexpectedly issued project requests".into());
            }
            None
        } else {
            Some(project::declaration(&self.requests)?)
        };
        let mut root_bindings = Vec::new();
        let mut product_selections = Vec::new();
        let mut exclusions = Vec::new();
        let mut native_bindings = Vec::new();
        for selection in self.host.bindings.take_executed()? {
            match selection {
                ExecutedBuildSelection::Entry(binding) => root_bindings.push(binding),
                ExecutedBuildSelection::Exclusion(selection) => exclusions.push(selection),
                ExecutedBuildSelection::Native(selection) => native_bindings.push(selection),
                ExecutedBuildSelection::Declaration(selection) => {
                    product_selections.push(selection)
                }
            }
        }
        let declared_product_selections = self.host.bindings.declared().to_vec();
        Ok(ReturnedBuild {
            identity: self.host.produced.view().custody_identity(),
            declaration,
            source_join: self.host.default_sources.clone(),
            requests: self.requests,
            demanded_sources: self.demanded_sources,
            settings,
            optimization_report,
            root_bindings,
            product_selections,
            native_bindings,
            native_formations: self.host.bindings.native_formations().to_vec(),
            exclusions,
            declared_product_selections,
            log: self.host.requests.take_log()?,
            filesystem: self.host.filesystem.take_effects()?,
        })
    }
}

fn describe(error: impl std::fmt::Debug) -> String {
    format!("Terminal Build: {error:?}")
}
