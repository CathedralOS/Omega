//! Admission: fixing the exact entry, target inputs and authority of one
//! build occurrence into an opaque `AdmittedBuildProgram` before anything
//! runs.

use crate::admission::configuration::BuildConfig;
use crate::admission::vocabulary;
use crate::admission::vocabulary::{
    build_reaches_filesystem_facet, has_exact_toolchain_build_facet, is_build_machine,
};
use crate::execute_admitted_build_program;
use crate::optimization;
use build_output::PackageGeneratedSource;
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineExecutionMode,
    BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use package_compilation::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
use package_compilation::{
    BuildCapturedSourceInventory, BuildEvaluationUsage, BuildObservationSummary,
};
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

/// The restricted host operation one admitted build activation requests.
///
/// Members are the normalized review vocabulary for restricted build-time
/// host reach (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
/// A new restricted operation lands here so package review can surface it
/// beside — never inside — the package's product-authority rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildOperation {
    /// Execute the admitted build machine against scoped real filesystem
    /// roots: reads land under granted read roots and writes under granted
    /// write roots, and anything else is refused before the host is touched.
    ScopedFilesystemExecution,
    /// Execute against the real filesystem without path grants. Build
    /// admission does not currently select this mode; it is named so a
    /// review cannot silently drop it if an evaluator ever carries it.
    UnscopedFilesystemExecution,
}

/// The logical grant root one restricted build request names. Compiler
/// vocabulary only — never the host path the root maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildGrantRoot {
    /// The package's source inventory.
    SourceInventory,
    /// The activation's staged build-output tree.
    StagedOutput,
    /// A compiler-issued grant root identity this projection does not name.
    Other(u32),
}

/// One logical root grant inside a restricted build request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildGrant {
    root: RestrictedBuildGrantRoot,
    /// The granted root's canonical metadata index narrows every operation
    /// to captured membership and kind.
    narrowed: bool,
    /// The immutable captured inventory bound to the root, when one exists.
    /// Reads then hit a fresh private materialization of that inventory,
    /// never the live source root.
    captured: Option<BuildCapturedSourceInventory>,
}

impl RestrictedBuildGrant {
    pub const fn root(&self) -> RestrictedBuildGrantRoot {
        self.root
    }

    pub const fn narrowed(&self) -> bool {
        self.narrowed
    }

    pub const fn captured(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured
    }
}

/// The bounds admission attached to one restricted build request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildBounds {
    /// The shared filesystem sponsor account's staging ceilings, present
    /// when a caller-installed sponsor binds every granted operation.
    filesystem_sponsor_limits: Option<checked_interpreter::FilesystemSponsorLimits>,
    /// The evaluation sponsor's resource ceilings, present when a
    /// caller-installed sponsor caps this activation's evaluator.
    evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
    /// The declared sealed outputs that must complete in the staged-output
    /// tree before this activation's result may publish.
    required_outputs: Vec<Vec<u8>>,
    /// Whether this activation publishes retained outputs only.
    artifact_only: bool,
}

impl RestrictedBuildBounds {
    pub const fn filesystem_sponsor_limits(
        &self,
    ) -> Option<checked_interpreter::FilesystemSponsorLimits> {
        self.filesystem_sponsor_limits
    }

    pub const fn evaluation_sponsor_limits(&self) -> Option<BuildEvaluationSponsorLimits> {
        self.evaluation_sponsor_limits
    }

    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn artifact_only(&self) -> bool {
        self.artifact_only
    }
}

/// One normalized restricted build-host request admitted on a build
/// activation (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
///
/// The request records, in compiler vocabulary, what the package's build
/// machine asks of the host before that authority executes: the restricted
/// operation, the logical resource roots it reaches, and the bounds
/// admission attached. It carries no host paths, live handles, or ephemeral
/// capabilities, so an installer may retain it as accepted-request meaning —
/// for example in a lock — without storing machine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildRequest {
    operation: RestrictedBuildOperation,
    read_grants: Vec<RestrictedBuildGrant>,
    write_grants: Vec<RestrictedBuildGrant>,
    bounds: RestrictedBuildBounds,
    build_execution_profile: Option<target::TargetProfile>,
    selected_target_profile: Option<target::TargetProfile>,
}

impl RestrictedBuildRequest {
    pub const fn operation(&self) -> RestrictedBuildOperation {
        self.operation
    }

    /// Logical roots this request may read, in grant order.
    pub fn read_grants(&self) -> &[RestrictedBuildGrant] {
        &self.read_grants
    }

    /// Logical roots this request may write, in grant order.
    pub fn write_grants(&self) -> &[RestrictedBuildGrant] {
        &self.write_grants
    }

    pub const fn bounds(&self) -> &RestrictedBuildBounds {
        &self.bounds
    }

    /// The admitted build execution profile this request's build-scope
    /// declarations were checked against. `None` names an admitted host no
    /// catalogued profile describes.
    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }

    /// The product target the requesting compilation realizes with this
    /// request. The Build evaluates once for every target; each realized
    /// target's compilation records its own copy (`for_target`).
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    /// This request as recorded by one realized target's compilation.
    #[must_use]
    pub const fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.selected_target_profile = profile;
        self
    }
}

pub(crate) struct SelectedAdmittedBuildMachine {
    pub(crate) entry: PreparedBuildMachineEntry,
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) normalized_callable_identity: String,
    pub(crate) optimization_admission: optimization::BuildOptimizationAdmission,
    pub(crate) has_toolchain_build_activation: bool,
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
    /// compiler vocabulary — never the host paths or live handles it binds —
    /// so a consumer may retain accepted request meaning separately from the
    /// actual invocation grants.
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        let AdmittedBuildMachine::Selected(selected) = &self.machine else {
            return Vec::new();
        };
        restricted_build_requests(
            &self.filesystem_scope,
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
    let optimization_admission = optimization::BuildOptimizationAdmission::admit(typed)?;
    let has_toolchain_build_activation =
        build_time_evaluation::admit_toolchain_build_activation(typed)?;

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

    let mut build_fields = Vec::new();
    if has_toolchain_build_activation {
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
    // Granular privileged-service grants (wiki/spec/build/permissions.md
    // #privileged-services): every flag is false until the root build machine
    // assigns it, so a hosted image admits no mediated privileged class.
    if has_exact_toolchain_build_facet(typed, "PrivilegedServices") {
        build_fields.push((
            "privileged_services".to_owned(),
            BuildTimeValue::Struct {
                type_name: "PrivilegedServices".to_owned(),
                fields: vec![
                    ("port_io".to_owned(), BuildTimeValue::Bool(false)),
                    ("interrupt_table".to_owned(), BuildTimeValue::Bool(false)),
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
    if has_exact_toolchain_build_facet(typed, "BuildInputs") {
        build_fields.push(("inputs".to_owned(), filesystem_scope.named_input_facet()));
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
            optimization_admission,
            has_toolchain_build_activation,
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

/// Project the admitted authority decision into normalized restricted
/// build-host requests.
///
/// Virtual execution and compiler-owned captured-input/private-staging work
/// require no restricted-action decision. Other real host reach remains
/// restricted, including scoped access without private staging custody.
/// Logical roots are reported by compiler
/// vocabulary identity — the host paths the roots map to never enter the
/// projection.
fn restricted_build_requests(
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    artifact_only: bool,
    execution_mode: &BuildMachineExecutionMode,
) -> Vec<RestrictedBuildRequest> {
    let BuildMachineExecutionMode::Granted { filesystem, .. } = execution_mode else {
        return Vec::new();
    };
    if filesystem_scope.is_private_snapshot_execution(filesystem) {
        return Vec::new();
    }
    let (operation, read_roots, write_roots, filesystem_sponsor) = match filesystem {
        BuildMachineFilesystemAccess::RealScoped(grants) => (
            RestrictedBuildOperation::ScopedFilesystemExecution,
            &grants.read_roots[..],
            &grants.write_roots[..],
            None,
        ),
        BuildMachineFilesystemAccess::RealScopedSponsored { grants, sponsor } => (
            RestrictedBuildOperation::ScopedFilesystemExecution,
            &grants.read_roots[..],
            &grants.write_roots[..],
            Some(sponsor),
        ),
        BuildMachineFilesystemAccess::RealUnscoped => (
            RestrictedBuildOperation::UnscopedFilesystemExecution,
            &[][..],
            &[][..],
            None,
        ),
        BuildMachineFilesystemAccess::Virtual => return Vec::new(),
    };
    let captured = filesystem_scope.captured_source_inventory();
    let grant = |grant_root: &BuildMachineFilesystemGrantRoot| {
        let identity = grant_root.identity();
        let (root, captured) = if identity == BUILD_SOURCE_ROOT_IDENTITY {
            (RestrictedBuildGrantRoot::SourceInventory, captured)
        } else if identity == BUILD_OUTPUT_ROOT_IDENTITY {
            (RestrictedBuildGrantRoot::StagedOutput, None)
        } else {
            (
                RestrictedBuildGrantRoot::Other(identity.get()),
                filesystem_scope.named_input_inventory(identity),
            )
        };
        RestrictedBuildGrant {
            root,
            narrowed: grant_root.canonical_metadata().is_some(),
            captured,
        }
    };
    vec![RestrictedBuildRequest {
        operation,
        read_grants: read_roots.iter().map(&grant).collect(),
        write_grants: write_roots.iter().map(&grant).collect(),
        bounds: RestrictedBuildBounds {
            filesystem_sponsor_limits: filesystem_sponsor.and_then(|sponsor| sponsor.limits().ok()),
            evaluation_sponsor_limits: evaluation_sponsor.map(BuildEvaluationSponsor::limits),
            required_outputs: filesystem_scope
                .required_outputs()
                .iter()
                .cloned()
                .collect(),
            artifact_only,
        },
        build_execution_profile: filesystem_scope.activation().build_execution_profile(),
        selected_target_profile: None,
    }]
}
