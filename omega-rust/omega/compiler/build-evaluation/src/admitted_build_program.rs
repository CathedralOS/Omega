//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use crate::admission::configuration::BuildConfig;
use crate::execute_admitted_build_program;
use build_output::PackageGeneratedSource;
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildMachineFilesystemMetadataLayout,
    BuildTimeValue, PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::build_value::{
    BuildValueAdmission, build_reaches_filesystem_facet, is_build_machine,
};
use package_compilation::{BuildEvaluationUsage, BuildObservationSummary};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedBuildConfig {
    pub config: BuildConfig,
    pub optimization_report_request: optimization_core::OptimizationReportRequest,
    pub evaluation_usage: Option<BuildEvaluationUsage>,
    pub observation_summary: Option<BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub generated_sources: Vec<PackageGeneratedSource>,
}

/// Whether one admitted build activation has an executable build machine.
///
/// The selected entry itself is deliberately absent from this public
/// projection. It remains coupled to the prepared program inside
/// [`AdmittedBuildProgram`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedBuildProgramDisposition {
    NoBuildMachine,
    SelectedBuildMachine,
}

/// Compiler-owned authority decision retained by an admitted build program.
///
/// This is an audit projection of the exact interpreter mode held by the
/// checkpoint. It cannot be used to construct or replace that mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedBuildAuthorityVerdict {
    NoBuildMachine,
    Pure,
    Granted,
}

pub(crate) struct SelectedAdmittedBuildMachine {
    pub(crate) entry: PreparedBuildMachineEntry,
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) normalized_callable_identity: String,
    pub(crate) value_admission: BuildValueAdmission,
    pub(crate) filesystem_reachable: bool,
    pub(crate) execution_mode: BuildMachineExecutionMode,
    pub(crate) initial_build: BuildTimeValue,
}

pub(crate) enum AdmittedBuildMachine {
    None,
    Selected(SelectedAdmittedBuildMachine),
}

/// Opaque activation-local checkpoint for build-machine execution.
///
/// Preparation, exact entry selection, reach inference, target admission, and
/// authority validation all finish before this value is issued. The prepared
/// program and its entry token never leave the carrier independently, so a
/// caller cannot replace either half before execution.
///
/// The custody boundary is enforced by privacy, not a convention:
///
/// ```compile_fail
/// use build_evaluation::AdmittedBuildProgram;
/// use build_time_evaluation::PreparedBuildMachineProgram;
///
/// fn substitute_program(
///     admitted: &mut AdmittedBuildProgram,
///     foreign: PreparedBuildMachineProgram,
/// ) {
///     admitted.prepared = foreign;
/// }
/// ```
#[must_use = "an admitted build program must be consumed by execute"]
pub struct AdmittedBuildProgram {
    pub(crate) prepared: PreparedBuildMachineProgram,
    pub(crate) machine: AdmittedBuildMachine,
    pub(crate) operational_plan: flow_effects::OperationalPlan,
    pub(crate) service_reach_plan: flow_effects::ServiceReachInferencePlan,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    /// The validated `builder.artifact_only()` application modifier. An
    /// artifact-only activation publishes retained outputs only: it may not
    /// bind executable roots or select boundary providers, and it must
    /// complete at least one required-output obligation.
    pub(crate) artifact_only: bool,
}

impl AdmittedBuildProgram {
    pub fn disposition(&self) -> AdmittedBuildProgramDisposition {
        match &self.machine {
            AdmittedBuildMachine::None => AdmittedBuildProgramDisposition::NoBuildMachine,
            AdmittedBuildMachine::Selected(_) => {
                AdmittedBuildProgramDisposition::SelectedBuildMachine
            }
        }
    }

    pub fn authority_verdict(&self) -> AdmittedBuildAuthorityVerdict {
        match &self.machine {
            AdmittedBuildMachine::None => AdmittedBuildAuthorityVerdict::NoBuildMachine,
            AdmittedBuildMachine::Selected(selected) => match &selected.execution_mode {
                BuildMachineExecutionMode::Pure => AdmittedBuildAuthorityVerdict::Pure,
                BuildMachineExecutionMode::Granted { .. } => AdmittedBuildAuthorityVerdict::Granted,
            },
        }
    }

    pub const fn selected_build_machine_symbol(&self) -> Option<SymbolHandle> {
        match &self.machine {
            AdmittedBuildMachine::None => None,
            AdmittedBuildMachine::Selected(selected) => Some(selected.symbol),
        }
    }

    pub fn selected_build_machine_callable_identity(&self) -> Option<&str> {
        match &self.machine {
            AdmittedBuildMachine::None => None,
            AdmittedBuildMachine::Selected(selected) => {
                Some(&selected.normalized_callable_identity)
            }
        }
    }

    pub const fn initial_build_snapshot(&self) -> Option<&BuildTimeValue> {
        match &self.machine {
            AdmittedBuildMachine::None => None,
            AdmittedBuildMachine::Selected(selected) => Some(&selected.initial_build),
        }
    }

    pub const fn operational_plan(&self) -> &flow_effects::OperationalPlan {
        &self.operational_plan
    }

    pub const fn service_reach_plan(&self) -> &flow_effects::ServiceReachInferencePlan {
        &self.service_reach_plan
    }

    /// The normalized restricted build-host requests this activation asks of
    /// the host, in issue order. They describe the admitted authority in
    /// package vocabulary — never the host paths or live handles it binds —
    /// so a consumer may retain accepted request meaning separately from the
    /// actual invocation grants.
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        let AdmittedBuildMachine::Selected(selected) = &self.machine else {
            return Vec::new();
        };
        self.filesystem_scope.restricted_build_requests(
            self.evaluation_sponsor.as_ref(),
            self.artifact_only,
            &selected.execution_mode,
        )
    }

    /// Consume the exact admitted program through one evaluation.
    pub fn execute(self) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
        execute_admitted_build_program(self)
    }
}

/// Prepare and admit the program's exact build-machine activation.
///
/// This stage performs every selection and authority decision but executes no
/// authored code. The returned carrier owns the only entry token accepted by
/// its prepared program.
pub fn admit_build_program(
    typed: &TypedTrees,
    build_source_id: Option<source::SourceId>,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    artifact_only: bool,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let prepared = PreparedBuildMachineProgram::prepare(typed)?;
    let typed = prepared.typed();
    let operational_plan = validation::infer_operational_may(typed);
    let service_reach_plan = validation::infer_service_reaches(typed, &operational_plan);

    let mut build_machines = typed
        .machines()
        .iter()
        .filter(|machine| is_build_machine(typed, machine, build_source_id));
    let Some(machine) = build_machines.next() else {
        return Ok(AdmittedBuildProgram {
            prepared,
            machine: AdmittedBuildMachine::None,
            operational_plan,
            service_reach_plan,
            filesystem_scope: filesystem_scope.clone(),
            evaluation_sponsor: evaluation_sponsor.cloned(),
            artifact_only,
        });
    };
    if let Some(second) = build_machines.next() {
        return Err(vec![Diagnostic::error(format!(
            "two build machines exist (`{}` and `{}`); a program declares at most one",
            machine.name.as_str(),
            second.name.as_str(),
        ))]);
    }
    let machine_symbol = machine.symbol;
    let machine_name = machine.name.as_str().to_owned();
    let machine_entry = prepared.entry(machine_symbol).map_err(|reason| {
        vec![Diagnostic::error(format!(
            "could not bind the selected build machine to its prepared program: {reason}"
        ))]
    })?;
    let normalized_callable_identity = typed
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity().to_owned())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected build machine has no canonical callable identity",
            )]
        })?;
    let value_admission = BuildValueAdmission::admit(typed)?;

    // Build authority comes only from compiler-owned Build facets. Runtime
    // boundary services remain ordinary program authority and are never
    // admitted merely because a build machine reaches them.
    let transitive = service_reach_plan
        .for_machine(machine.symbol)
        .map(|entry| service_reach_plan.services(entry.inferred_transitive))
        .unwrap_or(&[]);
    let transitive_names = transitive
        .iter()
        .map(|service| {
            typed
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect::<Vec<_>>();
    if std::env::var_os("OMEGA_DEBUG_BUILD_CONFIG").is_some() {
        eprintln!(
            "BUILDCFG: machine `{}` found, inferred transitive service reach [{}]",
            machine.name.as_str(),
            transitive_names.join(", "),
        );
    }
    if !transitive_names.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches boundary service{} `{}` -- build.omg may not reach \
             runtime boundary services; use the compiler-owned Build facets",
            if transitive_names.len() == 1 { "" } else { "s" },
            transitive_names.join(", "),
        ))]);
    }
    let filesystem_reachable =
        build_reaches_filesystem_facet(typed, &operational_plan, machine.symbol);

    let zero_build = value_admission.initial_value(typed, machine, filesystem_scope);

    // Omega owns the grant decision. Psi owns the target-neutral interpreter
    // entry selected by that explicit mode. BuildLog remains available in
    // either mode without granting a runtime boundary service.
    let execution_mode = if !filesystem_reachable {
        BuildMachineExecutionMode::Pure
    } else {
        let filesystem = filesystem_scope.establish_filesystem_access()?;
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout::default(),
        }
    };
    Ok(AdmittedBuildProgram {
        prepared,
        machine: AdmittedBuildMachine::Selected(SelectedAdmittedBuildMachine {
            entry: machine_entry,
            symbol: machine_symbol,
            name: machine_name,
            normalized_callable_identity,
            value_admission,
            filesystem_reachable,
            execution_mode,
            initial_build: zero_build,
        }),
        operational_plan,
        service_reach_plan,
        filesystem_scope: filesystem_scope.clone(),
        evaluation_sponsor: evaluation_sponsor.cloned(),
        artifact_only,
    })
}
