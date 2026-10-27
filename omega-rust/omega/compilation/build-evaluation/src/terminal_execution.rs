//! Retain one source-issued Build activation and its original receiver.
//!
//! This is the execution owner, not source discovery or package resolution.
//! The caller advances execution, receives requests between fuel slices, and must join
//! them before publishing a product. Both protocol adapters borrow one activation;
//! a pause never reconstructs the instance or replays earlier effects.

mod project;
mod provider_installation;
mod source_join;
pub use source_join::BuildDependencySelection;

use build_time_evaluation::BuildTimeValue;
use evaluation_accounting::EvaluationInvocationAccount;
use language_semantics::declaration_selection::{
    ExecutedBuildSelection, ExecutedProductSelection, ExecutedRootBinding,
    ProductDeclarationRequest,
};
use package_compilation::build_execution::build_value::{
    BuildValueAdmission, EvaluatedBuildSettings,
};
use psi_compiler::ProducedTerminalProgram;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstance, TerminalInstantiation,
    TerminalProgram, TerminalStructuralInputs, TerminalStructuralValue,
};

use crate::terminal_bindings::TerminalBuildBindingHost;
use crate::terminal_requests::{IssuedBuildRequest, TerminalBuildRequestHost};

pub(crate) fn measurement() -> evaluation_accounting::EvaluationMeasurementIdentity {
    evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
    }
}

/// Normal-return values and the complete request ledger. The coordinator's
/// mandatory dependency join precedes normalized Build publication.
pub struct ReturnedBuild {
    pub(crate) identity: terminal_codec::TerminalExecutionIdentity,
    pub(crate) declaration: build_declarations::BuildDeclaration,
    pub(crate) source_join:
        Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
    pub(crate) requests: Vec<IssuedBuildRequest>,
    pub(crate) demanded_sources: Vec<BuildDependencySelection>,
    pub(crate) settings: EvaluatedBuildSettings,
    pub(crate) optimization_report: optimization_core::OptimizationReportRequest,
    pub(crate) root_bindings: Vec<ExecutedRootBinding>,
    pub(crate) product_selections: Vec<ExecutedProductSelection>,
    pub(crate) exclusions: Vec<language_semantics::declaration_selection::ExecutedEnumExclusion>,
    pub(crate) declared_product_selections: Vec<std::sync::Arc<ProductDeclarationRequest>>,
    pub(crate) log: crate::terminal_requests::BuildLog,
    pub(crate) filesystem: Option<package_compilation::build_execution::BuildFilesystemEffects>,
}

impl ReturnedBuild {
    /// The role established by this activation's normal return. Package
    /// management may now bind its name to acquired source lineage; this does
    /// not settle dependency requests or authorize Product publication.
    pub fn declaration(&self) -> &build_declarations::BuildDeclaration {
        &self.declaration
    }

    /// The complete issued roster, including requests observed in earlier steps.
    /// Observing or ignoring a step cannot remove a dependency from final joining.
    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    /// Exact dependencies used during this activation, for helper execution or
    /// declaration queries. Catalog observations retain these demands without
    /// claiming completion of unrelated Product requests.
    pub fn demanded_sources(&self) -> &[BuildDependencySelection] {
        &self.demanded_sources
    }
}

/// Host custody outlives the interpreter's borrowed adapters. One host starts
/// one activation; dropping a paused activation cannot start its effects again.
pub struct TerminalBuildHost<'code> {
    produced: &'code ProducedTerminalProgram,
    account: &'code EvaluationInvocationAccount,
    original: TerminalStructuralValue,
    requests: TerminalBuildRequestHost<'code>,
    bindings: TerminalBuildBindingHost<'code>,
    filesystem: crate::terminal_filesystem::TerminalBuildFilesystemHost<'code>,
    started: bool,
    value_admission: BuildValueAdmission,
    initial: &'code BuildTimeValue,
}

impl<'code> TerminalBuildHost<'code> {
    pub(crate) fn new(
        produced: &'code ProducedTerminalProgram,
        account: &'code EvaluationInvocationAccount,
        machine: &'code package_compilation::build_execution::SelectedBuildMachine,
        filesystem_scope: &'code package_compilation::BuildMachineFilesystemScope,
    ) -> Result<Self, String> {
        let artifact = produced.view();
        let program = TerminalProgram::borrow(artifact);
        let entry = program.entry().map_err(describe)?;
        let [parameter] = entry.structural_parameters else {
            return Err("Build activation requires its one original structural receiver".into());
        };
        if !entry.scalar_parameters.is_empty() {
            return Err("Build activation has unexpected scalar parameters".into());
        }
        // Identity is invocation-local: neither a second instance nor a same-typed
        // guest constructor inherits this host's right to submit requests.
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
        let filesystem = crate::terminal_filesystem::TerminalBuildFilesystemHost::new(
            produced,
            &original,
            account,
            filesystem_scope,
            &machine.name,
        )?;
        Ok(Self {
            produced,
            account,
            original,
            requests,
            bindings,
            filesystem,
            started: false,
            value_admission: machine.value_admission,
            initial: &machine.initial_build,
        })
    }

    pub fn start(&mut self) -> Result<TerminalBuildActivation<'_, 'code>, String> {
        if self.started {
            return Err("Build host has already started its activation".into());
        }
        self.started = true;
        artifacts::compile_timings::counters::record_build_execution();
        let artifact = self.produced.view();
        let program = TerminalProgram::borrow(artifact);
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
        for call in binding_calls {
            if let language_semantics::declaration_selection::BuildSelectionRequest::ProviderReadiness(request) = &call.request {
                provider_slots.extend_from_slice(&request.requirement.boundaries);
            }
        }
        provider_slots.sort_unstable();
        provider_slots.dedup();
        let mut host_imports = request_calls
            .iter()
            .map(|call| call.boundary)
            .chain(binding_calls.iter().map(|call| call.boundary))
            .chain(query_calls.iter().map(|call| call.boundary))
            .chain(filesystem_calls.iter().map(|call| call.boundary))
            .collect::<Vec<_>>();
        host_imports.sort_unstable();
        host_imports.dedup();
        let mut supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                host_imports: &host_imports,
                installation: None,
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
                account: Some(self.account),
                ..Default::default()
            })
            .map_err(describe)?;
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
            if request_calls.iter().any(|call| call.boundary == boundary) {
                instance
                    .bind_import(boundary, self.requests.adapter(declaration.into()))
                    .map_err(describe)?;
            } else if binding_calls.iter().any(|call| call.boundary == boundary)
                || query_calls.iter().any(|call| call.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.bindings.adapter(declaration.into()))
                    .map_err(describe)?;
            } else if filesystem_calls
                .iter()
                .any(|call| call.boundary == boundary)
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

        let fuel =
            TerminalFuelMeter::with_invocation(self.account.clone(), Some(0)).map_err(describe)?;
        Ok(TerminalBuildActivation {
            instance,
            host: self,
            fuel,
            state: ActivationState::Running,
            requests: Vec::new(),
            demanded_sources: Vec::new(),
        })
    }
}

enum ActivationState {
    Running,
    Returned,
    Refused,
}

/// One interpreter instance, including its suspended continuation and shared
/// account. The caller owns scheduling between advances; no callback hides it.
pub struct TerminalBuildActivation<'host, 'code> {
    instance: TerminalInstance<'host, 'code>,
    host: &'host TerminalBuildHost<'code>,
    fuel: TerminalFuelMeter,
    state: ActivationState,
    requests: Vec<IssuedBuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
}

pub struct TerminalBuildStep<'step> {
    pub requests: &'step [IssuedBuildRequest],
    pub returned: bool,
    pub awaiting_provider: Option<crate::terminal_bindings::BuildProviderRequest>,
    pub awaiting_query: Option<std::sync::Arc<crate::terminal_bindings::BuildProductQuery>>,
}

impl<'host, 'code> TerminalBuildActivation<'host, 'code> {
    /// Resume only the exact demand that paused this activation. A failed answer
    /// poisons it rather than allowing later effects to conceal query failure.
    pub fn answer_query(
        &mut self,
        request: &crate::terminal_bindings::BuildProductQuery,
        answer: crate::terminal_bindings::BuildProductDescription,
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

    /// Complete immutable ledger issued so far, including earlier fuel slices.
    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    /// The exact receiving code for independently checked provider admission.
    pub fn view(&self) -> terminal_codec::TerminalExecutionView<'code> {
        self.host.produced.view()
    }

    /// Install every demanded slot before completing the retained readiness call.
    /// Partial failure poisons this activation; it cannot resume with half a provider.
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
        let bindings = provider_installation::prepare(self.view(), &request, provider)?;
        if !self.host.bindings.has_installed_backing(provider)? {
            for binding in bindings {
                self.instance.bind_component(binding)?;
            }
        }
        self.host.bindings.provider_installed(provider)?;
        self.demanded_sources.push(selected);
        self.state = ActivationState::Running;
        Ok(())
    }

    pub fn advance(&mut self, additional_fuel: u64) -> Result<TerminalBuildStep<'_>, String> {
        match self.state {
            ActivationState::Returned => return Err("Build activation has already returned".into()),
            ActivationState::Refused => return Err("Build activation has already failed".into()),
            ActivationState::Running => {}
        }
        // Failure is sticky, including an account or host-drain refusal. A slice
        // replenishes scheduling fuel, never the total invocation allowance.
        self.state = ActivationState::Refused;
        self.fuel.replenish(additional_fuel).map_err(describe)?;
        let status = self
            .instance
            .invoke(&mut self.fuel)
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

    /// Extract only a normal return. Consuming the activation closes its instance
    /// before retiring the host selections and log. The complete request ledger
    /// remains attached; this operation does not assert its dependencies joined.
    pub fn complete(mut self) -> Result<ReturnedBuild, String> {
        if !matches!(self.state, ActivationState::Returned) {
            return Err("Build completion requires its normal interpreter return".into());
        }
        let (settings, optimization_report) = crate::terminal_settings::extract(
            &self.instance,
            self.host.value_admission,
            self.host.initial,
        )?;
        self.instance.close().map_err(describe)?;
        drop(self.instance);
        let declaration = project::declaration(&self.requests)?;
        let mut root_bindings = Vec::new();
        let mut product_selections = Vec::new();
        let mut exclusions = Vec::new();
        for selection in self.host.bindings.take_executed()? {
            match selection {
                ExecutedBuildSelection::Entry(binding) => root_bindings.push(binding),
                ExecutedBuildSelection::Exclusion(selection) => exclusions.push(selection),
                ExecutedBuildSelection::Declaration(selection) => {
                    product_selections.push(selection)
                }
            }
        }
        let declared_product_selections = self.host.bindings.declared().to_vec();
        Ok(ReturnedBuild {
            identity: self.host.produced.view().custody_identity(),
            declaration,
            source_join: None,
            requests: self.requests,
            demanded_sources: self.demanded_sources,
            settings,
            optimization_report,
            root_bindings,
            product_selections,
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
