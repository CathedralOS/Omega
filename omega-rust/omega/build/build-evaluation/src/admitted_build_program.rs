//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use crate::admission::configuration::BuildConfig;
use crate::admission::vocabulary;
use crate::admission::vocabulary::{
    TargetBuildVocabulary, build_reaches_filesystem_facet, has_exact_toolchain_build_facet,
    is_build_machine, target_build_vocabulary, validate_immutable_build_target,
};
use crate::evidence::filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
use crate::evidence::observations::{BuildEvaluationUsage, BuildObservationSummary};
use crate::execute_admitted_build_program;
use crate::optimization;
use build_output::{CapturedBuildSourceInput, PackageGeneratedSource};
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use package_compilation::{BuildDependencyOccurrence, BuildSourceCaptureRequest};
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Invocation-level request to execute one build occurrence against a
/// captured immutable source snapshot and to require the named outputs to
/// complete as sealed regular files before the build result may publish.
///
/// The request is a plain carrier: it expresses intent but grants nothing by
/// itself. The caller binds the captured input inventory and the roster onto
/// the build's filesystem scope; admission then materializes the inventory's
/// fresh private snapshot and completion checking enforces the roster against
/// retained staged-output custody. Capture requires the source root's
/// canonical sealed form (the same physical inventory validated for canonical
/// Source metadata); an unsealed or mutating root fails capture rather than
/// producing a falsely attributed snapshot.
///
/// `dependency_inputs` carries immutable inputs already captured by the
/// caller, each keyed to an exact dependency occurrence — requester, purpose,
/// alias, and target — under a canonical slot name. Binding rejects an input
/// whose occurrence does not exist in the reconciled graph rather than
/// attaching it to another edge; a keyed input can never widen an occurrence
/// or substitute host filesystem access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSnapshotRequest {
    required_outputs: Vec<Vec<u8>>,
    capture: BuildSnapshotCapture,
    dependency_inputs:
        BTreeMap<BuildDependencyOccurrence, BTreeMap<Vec<u8>, CapturedBuildSourceInput>>,
}

/// The caller-authorized source inventory one build activation captures
/// before execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSnapshotCapture {
    /// The root package's complete sealed inventory — the exact custody the
    /// resolver already validated. Capture reads no member the package
    /// binding did not commit.
    PackageInventory,
    /// An explicit invocation inventory of files and subtrees for a
    /// standalone root. Capture reads exactly the declared members; a local
    /// build never implicitly exposes the working directory.
    Scoped(BuildSourceCaptureRequest),
}

impl BuildSnapshotRequest {
    /// A snapshot over the root package's sealed inventory. Package custody
    /// is itself the caller-authorized inventory — the resolver's canonical
    /// index fixes membership — so this form needs no capture request.
    pub fn new(required_outputs: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::PackageInventory,
            dependency_inputs: BTreeMap::new(),
        }
    }

    /// A snapshot over an explicit invocation inventory for a standalone
    /// root: exactly the files and subtrees the caller declared.
    pub fn scoped(
        required_outputs: impl IntoIterator<Item = Vec<u8>>,
        capture: BuildSourceCaptureRequest,
    ) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::Scoped(capture),
            dependency_inputs: BTreeMap::new(),
        }
    }

    /// Assign caller-captured immutable inputs to exact dependency
    /// occurrences. Each name is an input slot in canonical relative form,
    /// not a host path; a duplicate (occurrence, name) pair or a
    /// noncanonical name rejects the request.
    pub fn with_dependency_inputs(
        mut self,
        inputs: impl IntoIterator<Item = (BuildDependencyOccurrence, Vec<u8>, CapturedBuildSourceInput)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        for (occurrence, name, input) in inputs {
            if !checked_interpreter::canonical_filesystem_metadata_path_is_canonical(&name, false) {
                return Err(vec![Diagnostic::error(format!(
                    "named build input slot is not a canonical relative name: {name:?}"
                ))]);
            }
            if self
                .dependency_inputs
                .entry(occurrence)
                .or_default()
                .insert(name.clone(), input)
                .is_some()
            {
                return Err(vec![Diagnostic::error(format!(
                    "named build input slot {name:?} is declared twice for one dependency occurrence"
                ))]);
            }
        }
        Ok(self)
    }

    /// Required sealed output paths in canonical slash-separated form. An
    /// empty roster still runs against the captured source snapshot.
    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    /// The caller-authorized inventory this snapshot captures.
    pub const fn capture(&self) -> &BuildSnapshotCapture {
        &self.capture
    }

    /// Immutable inputs assigned to exact dependency occurrences.
    pub fn dependency_inputs(
        &self,
    ) -> impl Iterator<
        Item = (
            &BuildDependencyOccurrence,
            &BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
        ),
    > {
        self.dependency_inputs.iter()
    }
}

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

/// Exact target vocabulary admitted for one selected build activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedBuildTargetInputs {
    profile: target::TargetProfile,
    build_symbol: SymbolHandle,
    target_field_symbol: SymbolHandle,
    x86_deployment_features_field_symbol: SymbolHandle,
}

impl AdmittedBuildTargetInputs {
    pub const fn profile(self) -> target::TargetProfile {
        self.profile
    }

    pub const fn build_symbol(self) -> SymbolHandle {
        self.build_symbol
    }

    pub const fn target_field_symbol(self) -> SymbolHandle {
        self.target_field_symbol
    }

    pub const fn x86_deployment_features_field_symbol(self) -> SymbolHandle {
        self.x86_deployment_features_field_symbol
    }
}

pub(crate) struct SelectedAdmittedBuildMachine {
    pub(crate) entry: PreparedBuildMachineEntry,
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) normalized_callable_identity: String,
    pub(crate) optimization_admission: optimization::BuildOptimizationAdmission,
    pub(crate) target_vocabulary: Option<TargetBuildVocabulary>,
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
/// caller cannot replace either half before primary or replay execution.
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
    pub(crate) selected_target_profile: Option<target::TargetProfile>,
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

    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    pub fn selected_target_inputs(&self) -> Option<AdmittedBuildTargetInputs> {
        let AdmittedBuildMachine::Selected(selected) = &self.machine else {
            return None;
        };
        let vocabulary = selected.target_vocabulary?;
        let profile = self.selected_target_profile?;
        Some(AdmittedBuildTargetInputs {
            profile,
            build_symbol: vocabulary.build_symbol,
            target_field_symbol: vocabulary.target_field_symbol,
            x86_deployment_features_field_symbol: vocabulary.x86_deployment_features_field_symbol,
        })
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

    /// Consume the exact admitted program through primary evaluation and any
    /// verifier-owned replay.
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
    selected_target_profile: Option<target::TargetProfile>,
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
            selected_target_profile,
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
    let optimization_admission = optimization::BuildOptimizationAdmission::admit(typed)?;
    let target_vocabulary = target_build_vocabulary(typed, selected_target_profile)?;
    if let Some(target_vocabulary) = target_vocabulary {
        validate_immutable_build_target(typed, target_vocabulary)?;
    }

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

    // Replay evidence is bound to the activation that produced it: the
    // build's `Build.target`, root package occurrence, authored declaration
    // role, and admitted build execution profile are all observable inputs.
    // A record captured under a different activation is stale evidence for
    // this request.
    if let Some(bound_activation) = filesystem_scope.replay_activation() {
        let expected_activation = filesystem_scope.activation(selected_target_profile);
        if bound_activation != expected_activation {
            let mut drift = Vec::<&'static str>::new();
            if bound_activation.root_package_identity()
                != expected_activation.root_package_identity()
            {
                drift.push("root package identity");
            }
            if bound_activation.root_role() != expected_activation.root_role() {
                drift.push("root declaration role");
            }
            if bound_activation.selected_target_profile()
                != expected_activation.selected_target_profile()
            {
                drift.push("selected target profile");
            }
            if bound_activation.build_execution_profile()
                != expected_activation.build_execution_profile()
            {
                drift.push("build execution profile");
            }
            return Err(vec![Diagnostic::error(format!(
                "build filesystem replay record was captured for a different activation ({} drifted)",
                drift.join(", ")
            ))]);
        }
    }

    let mut build_fields = Vec::new();
    if let (Some(profile), Some(_)) = (selected_target_profile, target_vocabulary) {
        build_fields.push((
            "target".to_owned(),
            BuildTimeValue::Case {
                variant: profile.build_case_name().to_owned(),
                payload: Vec::new(),
            },
        ));
        build_fields.push((
            "x86_deployment_features".to_owned(),
            BuildTimeValue::Case {
                variant: "Baseline".to_owned(),
                payload: Vec::new(),
            },
        ));
    }
    build_fields.extend([
        (
            "subsystem".to_owned(),
            BuildTimeValue::Case {
                variant: "Console".to_owned(),
                payload: Vec::new(),
            },
        ),
        ("freestanding".to_owned(), BuildTimeValue::Bool(false)),
    ]);
    // The toolchain Build declares `identifier`; an authored Build opts in by
    // declaring the same field. Empty bytes mean no authored identity.
    if vocabulary::build_machine_declares_identifier_field(typed, machine) {
        build_fields.push(("identifier".to_owned(), BuildTimeValue::Text(Vec::new())));
    }
    if let Some(field) = optimization_admission.zero_build_field() {
        build_fields.push(field);
    }
    if has_exact_toolchain_build_facet(typed, "BuildLog") {
        build_fields.push((
            "log".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildLogFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    // The product facet is a compiler-issued marker: `builder.product.entry`
    // is the only product-selection route, and an authored `BuildProduct`
    // value never carries this private runtime identity.
    if has_exact_toolchain_build_facet(typed, "BuildProduct") {
        build_fields.push((
            "product".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildProductFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    // Optional proof-product requests (wiki/spec/proofs/publication.md). Both
    // flags are independent and false until the root build machine assigns
    // them; dependency metadata cannot enable or override the selection.
    if has_exact_toolchain_build_facet(typed, "Pcc") {
        build_fields.push((
            "pcc".to_owned(),
            BuildTimeValue::Struct {
                type_name: "Pcc".to_owned(),
                fields: vec![
                    ("psi".to_owned(), BuildTimeValue::Bool(false)),
                    ("native".to_owned(), BuildTimeValue::Bool(false)),
                ],
            },
        ));
    }
    if has_exact_toolchain_build_facet(typed, "BuildSource")
        && has_exact_toolchain_build_facet(typed, "BuildOutput")
    {
        let root_facet = |type_name: &str, root: BuildMachineFilesystemGrantRootIdentity| {
            BuildTimeValue::Struct {
                type_name: type_name.to_owned(),
                fields: vec![(
                    "root".to_owned(),
                    BuildTimeValue::Int(i64::from(root.get())),
                )],
            }
        };
        build_fields.extend([
            (
                "source".to_owned(),
                root_facet("$OmegaBuildSourceRoot", BUILD_SOURCE_ROOT_IDENTITY),
            ),
            (
                "output".to_owned(),
                root_facet("$OmegaBuildOutputRoot", BUILD_OUTPUT_ROOT_IDENTITY),
            ),
        ]);
    }
    let zero_build = BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: build_fields,
    };

    // Omega owns the grant decision. Psi owns the target-neutral interpreter
    // entry selected by that explicit mode. BuildLog remains available in
    // either mode without granting a runtime boundary service.
    let execution_mode = if !filesystem_reachable {
        BuildMachineExecutionMode::Pure
    } else {
        let filesystem = if filesystem_reachable {
            filesystem_scope.ensure_write_roots()?;
            filesystem_scope.ensure_canonical_source_metadata()?;
            filesystem_scope.ensure_captured_snapshot()?;
            filesystem_scope.filesystem_access()
        } else {
            BuildMachineFilesystemAccess::Virtual
        };
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
            optimization_admission,
            target_vocabulary,
            filesystem_reachable,
            execution_mode,
            initial_build: zero_build,
        }),
        operational_plan,
        service_reach_plan,
        filesystem_scope: filesystem_scope.clone(),
        evaluation_sponsor: evaluation_sponsor.cloned(),
        selected_target_profile,
        artifact_only,
    })
}
