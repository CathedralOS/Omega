//! Prepare one admitted Terminal Build and settle its exact normal-return output.
//! The caller owns execution stepping and dependency work between those operations.

use crate::{AdmittedBuildProgram, BuildConfig, ComputedBuildConfig};
use diagnostics::Diagnostic;
use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};

/// An independently owned invocation. Preparing dependencies may use the compiler
/// again while this code, sponsor account and admitted host scope remain alive.
pub struct PreparedBuildExecution {
    produced: psi_compiler::ProducedTerminalProgram,
    account: evaluation_accounting::EvaluationInvocationAccount,
    machine: package_compilation::build_execution::SelectedBuildMachine,
    filesystem_scope: package_compilation::BuildMachineFilesystemScope,
    host_issued: bool,
}

impl PreparedBuildExecution {
    pub(crate) fn prepare(admitted: AdmittedBuildProgram) -> Result<Self, Vec<Diagnostic>> {
        let AdmittedBuildProgram {
            prepared,
            machine,
            filesystem_scope,
            evaluation_sponsor,
            artifact_only: _,
        } = admitted;
        let refuse = |reason: String| {
            vec![Diagnostic::error(format!(
                "Terminal Build `{}` failed: {reason}",
                machine.name
            ))]
        };
        // Unconnected filesystem adapters cannot execute the legacy prelude's
        // placeholder bodies and then claim that the activation observed no I/O.
        if machine.filesystem_reachable {
            return Err(refuse(
                "Terminal Build filesystem operations are not installed".into(),
            ));
        }
        let checked = prepared
            .checked_trees()
            .ok_or_else(|| refuse("Build has no admitted source judgment".into()))?;
        let toolchain =
            source::source_identity::toolchain_source_identities(checked.symbols.source_files())
                .map_err(&refuse)?;
        let mut production = psi_compiler::TerminalProductionRequest::new(
            checked,
            psi_compiler::TerminalMachineSelection::Symbol(machine.symbol),
            &toolchain,
        );
        production.publication = terminal_codec::TerminalPublication::Invocation;
        let produced = production
            .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
            .map_err(|error| error.diagnostics())?;

        // These are Terminal units. An existing tree-measured sponsor is never
        // relabeled or silently replaced by a fresh account.
        let measurement = crate::terminal_execution::measurement();
        const INVOCATION_FUEL: u64 = 100_000;
        let sponsor = match evaluation_sponsor {
            Some(sponsor) => sponsor,
            None => {
                // Standalone compilation previously had only a per-invocation work
                // ceiling. Its private account adds measurement, not filesystem grants.
                let limits = BuildEvaluationSponsorLimits::new(
                    INVOCATION_FUEL,
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
            .begin_invocation(measurement, INVOCATION_FUEL)
            .map_err(|error| refuse(error.to_string()))?;
        Ok(Self {
            produced,
            account,
            machine,
            filesystem_scope,
            host_issued: false,
        })
    }

    /// Borrow this invocation's host once. It cannot be replaced to restart effects
    /// after the caller drops a paused or failed activation.
    pub fn host(&mut self) -> Result<crate::TerminalBuildHost<'_>, Vec<Diagnostic>> {
        if self.host_issued {
            return Err(self.refuse("Build invocation already issued its execution host"));
        }
        self.host_issued = true;
        crate::TerminalBuildHost::new(&self.produced, &self.account, &self.machine).map_err(
            |reason| {
                vec![Diagnostic::error(format!(
                    "Terminal Build `{}` failed: {reason}",
                    self.machine.name
                ))]
            },
        )
    }

    pub(crate) fn complete(
        self,
        completed: crate::ReturnedBuild,
    ) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        if !self.host_issued || completed.identity != self.produced.view().custody_identity() {
            return Err(self.refuse("Build completion belongs to another invocation"));
        }
        let declaration = &completed.declaration;
        // No Product may consume an outstanding edge. This refusal remains until
        // the application coordinator installs the real request/join protocol.
        if completed.requests.iter().any(|issued| {
            matches!(
                issued.request(),
                build_declarations::BuildRequest::Dependency { .. }
            )
        }) {
            return Err(self.refuse(
                "dependency requests require coordinator settlement before Product compilation",
            ));
        }
        if matches!(declaration, build_declarations::BuildDeclaration::Application(application) if application.artifact_only)
        {
            return Err(self.refuse("artifact-only Build requires completed output custody"));
        }
        let mut observations =
            package_compilation::CapturedBuildObservations::new(&self.filesystem_scope);
        observations.retain_build_log(
            completed
                .log
                .publish(&self.account)
                .map_err(|reason| self.refuse(&reason))?,
        )?;
        let observation_summary =
            observations.settle(self.machine.filesystem_reachable, &self.machine.name)?;
        let config = BuildConfig {
            settings: completed.settings,
            ..Default::default()
        };
        let usage = *self
            .account
            .snapshot()
            .map_err(|error| self.refuse(&error.to_string()))?
            .record();
        Ok(ComputedBuildConfig {
            config,
            root_binding_requests: completed.root_bindings,
            project_declaration: Some(completed.declaration),
            optimization_report_request: completed.optimization_report,
            evaluation_usage: Some(usage),
            observation_summary: Some(observation_summary),
            selected_build_machine_symbol: Some(self.machine.symbol),
            generated_sources: Vec::new(),
        })
    }

    fn refuse(&self, reason: &str) -> Vec<Diagnostic> {
        vec![Diagnostic::error(format!(
            "Terminal Build `{}` failed: {reason}",
            self.machine.name
        ))]
    }
}
