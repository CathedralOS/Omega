//! Retain one source-issued Build activation and its original receiver.
//!
//! This is the execution owner, not source discovery or package resolution.
//! The caller advances execution, receives requests between fuel slices, and must join
//! them before publishing a product. Both protocol adapters borrow one activation;
//! a pause never reconstructs the instance or replays earlier effects.

mod project;
mod source_join;
pub use source_join::BuildDependencySelection;

use build_time_evaluation::BuildTimeValue;
use evaluation_accounting::EvaluationInvocationAccount;
use language_semantics::declaration_selection::ExecutedRootBinding;
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
    pub(crate) settings: EvaluatedBuildSettings,
    pub(crate) optimization_report: optimization_core::OptimizationReportRequest,
    pub(crate) root_bindings: Vec<ExecutedRootBinding>,
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
        let bindings =
            TerminalBuildBindingHost::new(artifact, produced.build_bindings(), original.clone())?;
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
        let mut instance = program
            .instantiate(TerminalInstantiation {
                structural_inputs: TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&self.original),
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                account: Some(self.account),
                ..Default::default()
            })
            .map_err(describe)?;

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
        let filesystem_calls = self
            .produced
            .build_filesystem()
            .calls_for(artifact)
            .ok_or("Build filesystem calls lost their artifact custody")?;
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
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
            } else if binding_calls.iter().any(|call| call.boundary == boundary) {
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
}

pub struct TerminalBuildStep<'step> {
    pub requests: &'step [IssuedBuildRequest],
    pub returned: bool,
}

impl TerminalBuildActivation<'_, '_> {
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
        Ok(ReturnedBuild {
            identity: self.host.produced.view().custody_identity(),
            declaration,
            source_join: None,
            requests: self.requests,
            settings,
            optimization_report,
            root_bindings: self.host.bindings.take_executed()?,
            log: self.host.requests.take_log()?,
            filesystem: self.host.filesystem.take_effects()?,
        })
    }
}

fn describe(error: impl std::fmt::Debug) -> String {
    format!("Terminal Build: {error:?}")
}
