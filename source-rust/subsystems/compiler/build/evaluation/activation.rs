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
2. The project Build driver drives the activation in work slices via `advance(additional_work)`.
   Whenever `advance` pauses on a provider readiness demand (`AwaitingEffect`) or
   a Product declaration query (`HostComputationPending`), the activation stays
   suspended in place with its live `TerminalInstance` and `EvaluationInvocationAccount`
   intact while the project Build driver compiles the required dependency and resumes via
   `install_provider` or `answer_query`.
3. Every mutating transition (`advance`, `install_provider`, `answer_query`) sets
   `self.state = ActivationState::Refused` before performing fallible work and
   restores `Running` or `Returned` only on success, so any partial failure
   permanently poisons the activation rather than letting later steps conceal an
   incomplete provider installation or failed query.
4. On normal return (`complete`), we extract the plain settings from borrowed
   entry `0` (`build_settings::extract`), close the interpreter instance before
   draining host selections and logs, validate the authored project role
   (`project::declaration`), and return `ReturnedBuild` with its complete
   request and demanded-source ledgers attached for coordinator source-join
   settlement.
5. A dependency preparation can expose a new package to the remaining Build.
   Stop at the completed call, check the successor against the executed code,
   and give the interpreter its original storage and work meter. Only the final
   normal return settles the requests, source joins and filesystem effects.
*/

pub(crate) mod preparation;
mod project;
mod protocol;
mod provider_installation;
mod source_join;
mod source_result;
pub use source_join::BuildDependencySelection;
pub use source_result::BuildSourceResult;

use evaluation_accounting::EvaluationInvocationAccount;
use normalized_build_settings::EvaluatedBuildSettings;
use source_compiler::representations::declaration_selection::ProductEntryBinding;
use source_compiler::publication::ProducedTerminalProgram;
use source_compiler::representations::declaration_selection::ExecutedBuildSelection;
use source_compiler::representations::declaration_selection::ExecutedEnumExclusion;
use source_compiler::representations::declaration_selection::ExecutedNativeBinding;
use source_compiler::representations::declaration_selection::ExecutedProductSelection;
use source_compiler::representations::declaration_selection::NativeBindingFormation;
use source_compiler::representations::declaration_selection::ProductDeclarationRequest;
use std::sync::Arc;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstance, TerminalInstantiation,
    TerminalProgram, TerminalProjectedHostAtom, TerminalStructuralInputs, TerminalStructuralValue,
};
use evaluation_work::EvaluationWorkMeter;

use crate::host::bindings::TerminalBuildBindingHost;
use crate::host::requests::{IssuedBuildRequest, TerminalBuildRequestHost};

pub(crate) fn measurement() -> evaluation_contracts::EvaluationMeasurementIdentity {
    evaluation_contracts::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_contracts::TerminalStorageScheduleIdentity::CURRENT,
    }
}

fn require_source_acquisition(
    issued: Option<&package_compilation::CapturedLocalSource>,
    expected: &package_compilation::CapturedLocalSource,
) -> Result<(), String> {
    if issued != Some(expected) {
        return Err("Build declaration does not belong to this original source acquisition".into());
    }
    Ok(())
}

pub struct ReturnedBuild {
    pub(crate) history: evaluation_receipts::BuildExecutionHistory,
    pub(crate) helper_accounting:
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    account: EvaluationInvocationAccount,
    unbound_acquisition: Option<Arc<package_compilation::CapturedLocalSource>>,
    pub(crate) identity: terminal_codec::TerminalExecutionIdentity,
    pub(crate) declaration: Option<build_declarations::BuildDeclaration>,
    pub(crate) source_join: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    pub(crate) requests: Vec<IssuedBuildRequest>,
    pub(crate) demanded_sources: Vec<BuildDependencySelection>,
    pub(crate) settings: EvaluatedBuildSettings,
    pub(crate) optimization_report: optimization_requests::OptimizationReportRequest,
    pub(crate) root_bindings: Vec<ProductEntryBinding>,
    pub(crate) product_selections: Vec<ExecutedProductSelection>,
    pub(crate) native_formations: Vec<NativeBindingFormation>,
    pub(crate) native_bindings: Vec<ExecutedNativeBinding>,
    pub(crate) exclusions: Vec<ExecutedEnumExclusion>,
    pub(crate) declared_product_selections: Vec<Arc<ProductDeclarationRequest>>,
    pub(crate) output: crate::host::requests::BuildRequestOutput,
    pub(crate) filesystem: Option<package_compilation::build_execution::BuildFilesystemEffects>,
}

impl ReturnedBuild {
    pub(crate) fn original_acquisition(&self) -> Option<&package_compilation::CapturedLocalSource> {
        self.unbound_acquisition.as_deref()
    }

    /// Check that the receiving source acquisition owns this declaration and request ledger.
    pub fn require_source_acquisition(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> Result<(), String> {
        require_source_acquisition(self.original_acquisition(), source)
    }

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

// Empty Output values are ordinary assignable data. Only the host-supplied
// payload, retained separately from their stable storage home, authorizes use.
struct OriginalOutput {
    path: Vec<terminal_psi::StructuralPathSegment>,
    value: TerminalStructuralValue,
}

impl OriginalOutput {
    fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        original: &TerminalStructuralValue,
    ) -> Result<Self, String> {
        let home = crate::host::requests::build_facet(artifact, original, "output")?;
        let declaration = artifact
            .module()
            .types.structural_types
            .iter()
            .find(|declaration| declaration.id == home.structural_type)
            .ok_or("Build output facet lost its declaration")?;
        if !matches!(&declaration.shape, terminal_psi::StructuralTypeShape::Record { fields, .. } if fields.is_empty())
        {
            return Err("Build output facet requires an empty record declaration".into());
        }
        let relative = home
            .path
            .strip_prefix(original.path.as_slice())
            .ok_or("Build output facet lost its original receiver path")?;
        let (qualifications, projected_qualifications) =
            terminal_psi::StructuralPathQualification::select(
                &original.qualifications,
                &original.projected_qualifications,
                relative,
            );
        // This is the expected token for this stage's checked declaration.
        // Only the initial invocation places this token in guest storage.
        let mut value = TerminalStructuralValue::minted(
            crate::host::ORIGINAL_OUTPUT_IDENTITY,
            home.structural_type,
            qualifications,
        );
        value.projected_qualifications = projected_qualifications;
        Ok(Self {
            path: relative.to_vec(),
            value,
        })
    }

    fn binding(&self) -> TerminalProjectedHostAtom {
        TerminalProjectedHostAtom {
            argument_index: 0,
            path: self.path.clone(),
            value: self.value.clone(),
        }
    }
}

pub struct TerminalBuildHost {
    unbound_acquisition: Option<Arc<package_compilation::CapturedLocalSource>>,
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<ProducedTerminalProgram>,
    accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: EvaluationInvocationAccount,
    original: TerminalStructuralValue,
    output: OriginalOutput,
    requests: Arc<TerminalBuildRequestHost>,
    bindings: Arc<TerminalBuildBindingHost>,
    filesystem: Arc<crate::host::filesystem::TerminalBuildFilesystemHost>,
    started: std::sync::atomic::AtomicBool,
    execution_profile: Option<target::TargetProfile>,
    default_source: bool,
    default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
}

impl TerminalBuildHost {
    pub(crate) fn new(
        execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
        produced: Arc<ProducedTerminalProgram>,
        accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
        providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
        account: &EvaluationInvocationAccount,
        machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
        filesystem_scope: &package_compilation::BuildMachineFilesystemScope,
        execution_profile: Option<target::TargetProfile>,
        default_source: bool,
        default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
        unbound_acquisition: Option<Arc<package_compilation::CapturedLocalSource>>,
        carried_filesystem: Option<crate::host::filesystem::CarriedFilesystem>,
    ) -> Result<Arc<Self>, String> {
        let artifact = produced.view();
        let parameter = machine.receiver_for(artifact.module())?;
        let program = TerminalProgram::borrow(artifact);
        let entry = program.entry().map_err(describe)?;
        // The evaluator is the build entry's caller, so it owes the entry's
        // `requires` clauses before it runs anything. It hands the entry a
        // Build it promises nothing about, so the only clauses it can owe
        // are ones true of the evaluator itself, such as its address width
        // (`requires addr::Bound == 18446744073709551616`).
        match entry.unconditional_precondition()? {
            Some(true) => {}
            Some(false) => {
                return Err(
                    "the build entry's `requires` does not hold for this build evaluator".into(),
                );
            }
            None => {
                return Err(
                    "not supported yet: a build entry `requires` clause over its arguments; \
                     the evaluator establishes nothing about the Build it passes"
                        .into(),
                );
            }
        }
        // The evaluator is the only issuer of the Build it lends (guest code
        // cannot construct one), so the value it presents carries exactly the
        // memberships the entry's formal declares, its sealed fields' included.
        let mut original = TerminalStructuralValue::minted(
            1,
            parameter.structural_type,
            parameter.qualifications.clone(),
        );
        original.projected_qualifications = parameter.projected_qualifications.clone();
        let output = OriginalOutput::new(artifact, &original)?;
        let requests = TerminalBuildRequestHost::new(
            artifact,
            produced.build_requests(),
            original.clone(),
            Some(account),
        )?;
        let bindings = TerminalBuildBindingHost::new(
            artifact,
            produced.build_bindings(),
            produced.build_requests(),
            original.clone(),
            Some(account),
        )?;
        let filesystem = crate::host::filesystem::TerminalBuildFilesystemHost::new(
            &produced,
            &original,
            &output.value,
            account,
            filesystem_scope,
            &machine.name,
            carried_filesystem,
        )?;
        Ok(Arc::new(Self {
            unbound_acquisition,
            execution_purpose,
            produced,
            accepted,
            providers,
            account: account.clone(),
            original,
            output,
            requests,
            bindings,
            filesystem,
            started: std::sync::atomic::AtomicBool::new(false),
            execution_profile,
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
        let instance = self.instantiate(true)?;
        let work = evaluation_work::sponsored_meter(self.account.clone(), Some(0)).map_err(describe)?;
        Ok(TerminalBuildActivation {
            history: Default::default(),
            instance,
            host: Arc::clone(self),
            work,
            state: ActivationState::Running,
            cut: None,
            requests: Vec::new(),
            demanded_sources: Vec::new(),
            helper_accounting: Default::default(),
        })
    }

    /// Attach the newly admitted code to the activation parked at its preparation.
    pub(crate) fn resume(
        self: &Arc<Self>,
        carried: CarriedStage,
        previous_source: &source_compiler::publication::AcceptedTerminalSource,
    ) -> Result<TerminalBuildActivation, String> {
        if self.started.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err("Build host has already started its activation".into());
        }
        let CarriedStage {
            mut history, instance, host, old_call, new_call, work, requests, demanded_sources,
            mut helper_accounting, request_host, mut binding_host, filesystem: _,
        } = carried;
        let old_program = instance.program().clone();
        let mut successor = self.instantiate(false)?;
        let new_program = successor.program().clone();
        let stack = instance.completed_call_stack()?;
        if stack.last().map(|site| (site.machine, site.operation)) != Some(old_call) {
            return Err("the stopped call differs from the selected preparation".into());
        }
        let prefix = source_compiler::checked_build_stack(
            previous_source,
            old_program.module(),
            &stack,
            &self.accepted,
            new_program.module(),
            new_call,
        )?;
        let providers = protocol::check(
            &prefix,
            &host,
            self,
            &binding_host,
            previous_source,
            &self.accepted,
        )?;
        for (installation, request) in binding_host.installed_providers.iter_mut().zip(providers) {
            provider_installation::admit_request(
                self.produced.view(),
                &request,
                &installation.provider,
            )?;
            let profile = self
                .execution_profile
                .ok_or("resuming an installed provider requires its execution profile")?;
            for binding in provider_installation::prepare(
                self.produced.view(),
                &request,
                &installation.provider,
                profile,
                &self.account,
                &mut helper_accounting,
            )? {
                successor.bind_component(binding)?;
            }
            installation.request = request;
        }
        self.requests.adopt(request_host, &prefix)?;
        self.bindings.adopt(binding_host, &prefix)?;
        self.filesystem.resume_markers(&prefix)?;
        let instance = TerminalInstance::continue_checked_entry(instance, successor, &prefix)?;
        history.resumed(
            host.produced.view().custody_identity().semantic(),
            self.produced.view().custody_identity().semantic(),
            prefix
                .stack()
                .iter()
                .map(|(old, new)| ((old.machine, old.operation), (new.machine, new.operation))),
        )?;
        Ok(TerminalBuildActivation {
            history,
            instance,
            host: Arc::clone(self),
            work,
            state: ActivationState::Running,
            cut: None,
            requests,
            demanded_sources,
            helper_accounting,
        })
    }

    /// Only the first guest receives a fresh Output. A resumed guest retains
    /// the original payload in whichever checked storage currently owns it.
    fn instantiate(
        self: &Arc<Self>,
        issue_output: bool,
    ) -> Result<TerminalInstance<'static, 'static>, String> {
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
            if let source_compiler::representations::declaration_selection::BuildSelectionRequest::ProviderReadiness(request) = &call.request {
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
        let operations = self
            .providers
            .intrinsics
            .operations_for(program.execution_identity())
            .map_err(|reason| format!("Build selected intrinsic binding: {reason}"))?;
        host_imports.extend(operations.iter().map(|(slot, _)| *slot));
        host_imports.sort_unstable();
        host_imports.dedup();
        let mut supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                host_imports: &host_imports,
                external_completions: &[],
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
        let output = issue_output.then(|| self.output.binding());
        let mut instance = program
            .instantiate(TerminalInstantiation {
                float_evaluation:
                    terminal_interpreter::float_result_policy::FloatEvaluation::CompileTime,
                structural_inputs: TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&self.original),
                    projected_host_atoms: output.as_slice(),
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                installation: Some(&supplies),
                account: Some(&self.account),
                ..Default::default()
            })
            .map_err(describe)?;
        for &(slot, operation) in operations {
            interpreter_numeric_imports::bind_numeric_import_slot(&mut instance, slot, operation)
                .map_err(|reason| format!("Build selected intrinsic binding: {reason:?}"))?;
        }
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            if provider_slots.contains(&boundary) {
                continue;
            }
            let declaration = artifact
                .module()
                .interfaces.boundary_machines
                .iter()
                .find(|declaration| declaration.id == boundary)
                .ok_or("Build import has no Terminal declaration")?;
            if let Some(call) = request_calls
                .iter()
                .find(|call| call.site.boundary == boundary)
            {
                if call.action == source_compiler::publication::TerminalBuildRequestOperation::ConfigureTestFixture {
                    instance.bind_import(boundary, self.bindings.adapter(declaration.into()))
                } else {
                    instance.bind_import(boundary, self.requests.adapter(declaration.into()))
                }.map_err(describe)?;
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

        Ok(instance)
    }
}

/// What a checked program that completed its preparation leaves for the
/// next stage of the same activation.
type EntryCall = (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::OperationId,
);

pub(crate) struct CarriedStage {
    history: evaluation_receipts::BuildExecutionHistory,
    instance: TerminalInstance<'static, 'static>,
    host: Arc<TerminalBuildHost>,
    old_call: EntryCall,
    new_call: EntryCall,
    work: EvaluationWorkMeter,
    requests: Vec<IssuedBuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
    helper_accounting: source_compiler::compile_time_invocations::SourceCompletionAccounting,
    request_host: crate::host::requests::CarriedRequests,
    binding_host: crate::host::bindings::CarriedBindings,
    pub(crate) filesystem: Option<crate::host::filesystem::CarriedFilesystem>,
}

struct ArmedPreparations {
    can_return: bool,
    paths: Vec<Vec<source_compiler::BuildPreparationSite>>,
    calls: terminal_interpreter::TerminalEntryCalls,
}

enum ActivationState {    Running,
    AtPreparation(usize),
    Returned,
    Refused,
}

pub struct TerminalBuildActivation {
    history: evaluation_receipts::BuildExecutionHistory,
    instance: TerminalInstance<'static, 'static>,
    host: Arc<TerminalBuildHost>,
    work: EvaluationWorkMeter,
    state: ActivationState,
    cut: Option<ArmedPreparations>,
    requests: Vec<IssuedBuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
    helper_accounting: source_compiler::compile_time_invocations::SourceCompletionAccounting,
}

/// The single reason the interpreter returned control to the planner.
pub enum TerminalBuildStep {    Returned,
    AtPreparation(usize),
    NeedsWork,
    Provide(crate::host::bindings::BuildProviderRequest),
    Prepare(crate::host::requests::BuildDependencyPreparation),
    Query(Arc<crate::host::bindings::BuildProductQuery>),
}

impl TerminalBuildActivation {
    /// Check the owner before a live declaration names a subsequent Build stage.
    pub fn require_source_acquisition(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> Result<(), String> {
        require_source_acquisition(self.host.unbound_acquisition.as_deref(), source)
    }

    pub fn helper_evaluation_accounting(
        &self,
    ) -> &source_compiler::compile_time_invocations::SourceCompletionAccounting {
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

    /// A named query uses only the role already issued by this activation.
    /// This validates the current ledger; it does not complete the Build or
    /// promise that a later request will preserve the same declaration.
    pub fn project_declaration(&self) -> Result<build_declarations::BuildDeclaration, String> {
        project::declaration(&self.requests).map_err(|reason| format!("Product declaration query requires an already-issued named package or application: {reason}"))
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
        provider: &source_compiler::publication::ProducedBuildProvider,
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

    /// Answers the pause of `Build::prepare_dependency`: the coordinator
    /// acquired the awaited registration, ran its Build and prepared it as
    /// `selected`. The parked call completes on the next `advance`.
    pub fn dependency_prepared(
        &mut self,
        selected: BuildDependencySelection,
        inputs: [u8; 32],
        child_build_source: Option<[u8; 32]>,
        handoff: Option<[u8; 32]>,
    ) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Build dependency preparation requires a live readiness pause".into());
        }
        self.state = ActivationState::Refused;
        let awaited = self
            .host
            .requests
            .awaiting_preparation()?
            .ok_or("Build has no dependency preparation pause")?;
        if selected.purpose != awaited.purpose
            || selected.request_ordinal != awaited.request_ordinal
        {
            return Err(
                "prepared dependency differs from the registration the Build waits for".into(),
            );
        }
        self.host.requests.dependency_prepared()?;
        self.history.prepared(
            match selected.purpose {
                build_declarations::DependencyPurpose::Product => {
                    source_provenance::DependencyScope::Product
                }
                build_declarations::DependencyPurpose::Build => {
                    source_provenance::DependencyScope::Build
                }
            },
            selected.request_ordinal,
            selected.package,
            inputs,
            child_build_source,
            handoff,
        );
        if !self.demanded_sources.contains(&selected) {
            self.demanded_sources.push(selected);
        }
        self.state = ActivationState::Running;
        Ok(())
    }

    /// An authenticated return from the selected call is the only place to switch code.
    pub(crate) fn carry(
        mut self,
        path: &[source_compiler::BuildPreparationSite],
        new_call: EntryCall,
    ) -> Result<CarriedStage, String> {
        let Some(cut) = self.cut.take() else {
            return Err("the Build has no armed preparation".into());
        };
        let ActivationState::AtPreparation(index) = self.state else {
            return Err("the interpreter has not reached a checked preparation".into());
        };
        if cut.paths.get(index).map(Vec::as_slice) != Some(path)
            || !self.host.requests.completed_preparation()?
        {
            return Err("the successor does not continue the completed Build preparation".into());
        }
        let stack = self.instance.completed_call_stack()?;
        let old = stack
            .last()
            .ok_or("the parked Build lost its stopped call")?;
        Ok(CarriedStage {
            history: self.history,
            host: Arc::clone(&self.host),
            old_call: (old.machine, old.operation),
            new_call,
            instance: self.instance,
            work: self.work,
            requests: self.requests,
            demanded_sources: self.demanded_sources,
            helper_accounting: self.helper_accounting,
            request_host: self.host.requests.carry()?,
            binding_host: self.host.bindings.carry()?,
            filesystem: self.host.filesystem.carry()?,
        })
    }

    pub fn advance(&mut self, additional_work: u64) -> Result<TerminalBuildStep, String> {
        self.advance_diagnostic(additional_work)
            .map_err(|error| error.message)
    }

    /// Preserve a host refusal's authored call in the currently executing Build stage.
    pub fn advance_diagnostic(
        &mut self,
        additional_work: u64,
    ) -> Result<TerminalBuildStep, Box<diagnostics::Diagnostic>> {
        fn refusal(reason: impl Into<String>) -> Box<diagnostics::Diagnostic> {
            Box::new(diagnostics::Diagnostic::error(reason))
        }

        match self.state {
            ActivationState::Returned => return Err(refusal("Build activation has already returned")),
            ActivationState::AtPreparation(_) => return Err(refusal("a parked Build needs its checked successor")),
            ActivationState::Refused => return Err(refusal("Build activation has already failed")),
            ActivationState::Running => {}
        }
        self.state = ActivationState::Refused;
        if self.host.machine.preparation_allows_return().is_some() && self.cut.is_none() {
            return Err(refusal("an unfinished Build requires its checked preparation paths"));
        }
        self.work
            .replenish(additional_work)
            .map_err(|error| refusal(describe(error)))?;
        let status = match &self.cut {
            Some(cut) => self
                .instance
                .invoke_until_entry_calls(&cut.calls, &mut self.work),
            None => self.instance.invoke(&mut self.work).map(Some),
        }
        .map_err(|error| Box::new(self.guest_diagnostic(error)))?;
        self.requests
            .extend(self.host.requests.drain_requests().map_err(refusal)?);
        let Some(status) = status else {
            let index = self
                .instance
                .completed_call_path_index()
                .ok_or_else(|| refusal("the stopped interpreter has no completed preparation path"))?;
            self.state = ActivationState::AtPreparation(index);
            return Ok(TerminalBuildStep::AtPreparation(index));
        };
        let step = match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                if self.cut.as_ref().is_some_and(|cut| !cut.can_return) {
                    return Err(refusal("the Build returned before its checked preparation"));
                }
                TerminalBuildStep::Returned
            }
            TerminalExecutionStatus::SponsorExhausted(_) => TerminalBuildStep::NeedsWork,
            TerminalExecutionStatus::AwaitingEffect(_) => match (
                self.host.bindings.awaiting_provider().map_err(refusal)?,
                self.host.requests.awaiting_preparation().map_err(refusal)?,
            ) {
                (Some(provider), None) => TerminalBuildStep::Provide(provider),
                (None, Some(dependency)) => TerminalBuildStep::Prepare(dependency),
                (None, None) => {
                    return Err(refusal("Build paused without an admitted provider or dependency demand"));
                }
                (Some(_), Some(_)) => {
                    return Err(refusal("one Build effect requested both a provider and a dependency"));
                }
            },
            TerminalExecutionStatus::HostComputationPending(_) => {
                let query = self.host.bindings.awaiting_query().map_err(refusal)?
                    .ok_or_else(|| refusal("Build paused without an admitted Product query"))?;
                TerminalBuildStep::Query(query)
            }
            other => return Err(refusal(format!("Build did not return normally: {other:?}"))),
        };
        self.state = match &step {
            TerminalBuildStep::Returned => ActivationState::Returned,
            _ => ActivationState::Running,
        };
        Ok(step)
    }

    fn guest_diagnostic(
        &self,
        error: terminal_interpreter::TerminalInstanceError,
    ) -> diagnostics::Diagnostic {
        use terminal_interpreter::{TerminalInstanceError, TerminalInterpretError};
        if let TerminalInstanceError::Execution(TerminalInterpretError::ExecutionSite {
            machine,
            cause,
            ..
        }) = &error
            && let TerminalInterpretError::EffectRejected {
                operation,
                rejection,
            } = cause.as_ref()
        {
            let mut diagnostic = diagnostics::Diagnostic::error(&rejection.reason);
            let accepted = &self.host.accepted;
            if let Some(files) = accepted.source().symbols.source_map() {
                // Build services run in toolchain helpers. Walk the actual callers
                // so repeated uses of one helper still point at the call that ran.
                use terminal_psi::CallSite;
                diagnostic.source_span =
                    std::iter::once((*machine, CallSite::Operation(*operation)))
                        .chain(self.instance.caller_sites())
                        .find_map(|(machine, site)| {
                            let sources = accepted.closure().elaboration().sources.get(machine)?;
                            let span = match site {
                                CallSite::Operation(id) => sources.operations.get(id),
                                CallSite::Edge(id) => sources.edges.get(id),
                            }?
                            .span()?;
                            let file = files.file_at(span)?;
                            (!file.origin.is_toolchain()).then_some(span)
                        });
                package_compilation::attribute_diagnostics(
                    std::slice::from_mut(&mut diagnostic),
                    &files,
                    None,
                );
            }
            return diagnostic;
        }
        diagnostics::Diagnostic::error(describe(error))
    }

    pub fn complete(mut self) -> Result<ReturnedBuild, String> {
        if !matches!(self.state, ActivationState::Returned) {
            return Err("Build completion requires its normal interpreter return".into());
        }
        let (settings, optimization_report) =
            build_settings::extract(&self.instance, &self.host.machine)?;
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
        let declared_product_selections = self.host.bindings.declared()?;
        Ok(ReturnedBuild {
            history: self.history,
            helper_accounting: self.helper_accounting,
            account: self.host.account.clone(),
            unbound_acquisition: self.host.unbound_acquisition.clone(),
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
            native_formations: self.host.bindings.native_formations()?,
            exclusions,
            declared_product_selections,
            output: self.host.requests.take_output()?,
            filesystem: self.host.filesystem.take_effects()?,
        })
    }
}

fn describe(error: impl std::fmt::Debug) -> String {
    format!("Terminal Build: {error:?}")
}
