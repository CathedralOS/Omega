//! Build admission, execution, and checked result assembly.
//!
//! [`admit_build_program`] fixes the exact entry, target inputs, and authority
//! before issuing an opaque [`AdmittedBuildProgram`]. Its consuming execution
//! runs that checkpoint, checks replay and output custody, and assembles one
//! [`ComputedBuildConfig`]. Selection and evidence operations live with their
//! domain owners; this root retains the admission-to-result route.
//!
//! Build configuration (wiki/spec/build/configuration.md):
//! image facts come from `build.omg`'s augmenting machine, never from an
//! invented config grammar. When the program (build.omg is ordinary source,
//! auto-included next to main.omg) defines the conventionally-named free
//! machine `build(build: &mut Build)`, the compiler evaluates it at build time
//! (purity-gated, the L0 engine) with a ZII `Build` and reads the augmented
//! value back:
//!
//! ```omega
//! data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
//! data Optimization { case ControlFlowCleanup; /* ... */ }
//! data Optimizations { control_flow_cleanup: u8 in Trapping; /* ... */ }
//! data Build { subsystem: Subsystem; freestanding: bool; optimizations: Optimizations; }
//! machine build(build: &mut Build) {
//!     build.subsystem = Subsystem::EfiApplication;
//!     build.freestanding = true;
//!     build.optimizations.enable(Optimization::ControlFlowCleanup);
//! }
//! ```
//!
//! - `subsystem` is loader METADATA (a PE header u16 the compiler copies; it
//!   does not select the emitter). The ZII zero case is `Console` -- the
//!   correct default falls out of the type. `Unspecified(value)` is the
//!   escape hatch: any loader value a platform invents, with no compiler
//!   release.
//! - `freestanding` ("trust no host packages") is stated as itself --
//!   previously fused into the `efi_application` name.
//! - Absent build.omg == an empty `build` machine == the zero `Build`: the
//!   hosted console default.
//! - `optimizations` is an exact set of individually named transformations.
//!   It is empty by default; duplicates reject rather than acting like levels.
//! - `builder.roots.bind(target::ProgramEntry, Exact::machine);` executes
//!   through the original Build activation, including local helpers/reborrows.
//!   Executed requests rejoin their lexical owner before exact product selection.

mod configuration;
mod declarations;
mod filesystem_scope;
mod observation_identity;
mod observations;
mod optimization;
mod replay_eligibility;
mod replay_record;
mod selection;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod vocabulary;

pub use observation_identity::BuildObservationIdentity;

pub use replay_record::{
    BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits,
    ReviewOnlyBuildFilesystemReplayRecord, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};

use build_output::{
    PackageGeneratedSource, ReplayedBuildOutputEntry, empty, replayed_output_tree,
    select_included_sources,
};
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
pub use configuration::{BuildConfig, HostedApplicationIntent};

use configuration::extract_build_config;
pub use declarations::{WireCompatibilityDemand, harvest_provider_selections, harvest_root_grants};

use declarations::harvest_wire_compatibility_demands;
use diagnostics::Diagnostic;
pub use filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};

pub use observations::{
    BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION, BUILD_OBSERVATION_SCHEMA_VERSION,
    BuildCanonicalSourceMetadataIdentity, BuildEvaluationUsage, BuildFilesystemAuthorizedPath,
    BuildFilesystemByteOperand, BuildFilesystemGrantAccess, BuildFilesystemGrantRefusal,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleIdentity,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemLogicalHandleOutput,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservation,
    BuildFilesystemMetadataObservationKind, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemMutableI64Operand,
    BuildFilesystemMutableI64OperandResolution, BuildFilesystemObservedByteRegion,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationAttempt,
    BuildFilesystemOperationObservationClass, BuildFilesystemOperationResult,
    BuildFilesystemPathLikeOperand, BuildFilesystemProvider, BuildFilesystemReplayDisposition,
    BuildFilesystemReplayVerdict, BuildFilesystemReturnedPath,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildFilesystemScalarOperandValue, BuildIncludedSourceHandoff, BuildObservationClass,
    BuildObservationSummary,
};

use observations::{
    project_logical_handle_identity, project_logical_handle_input_resolution,
    project_logical_handle_kind, project_logical_handle_output_source, project_operation_result,
    project_scalar_operand_value,
};

use replay_eligibility::{
    ReceiptedOutputEntry, ReceiptedOutputFile, complete_no_output_failure_suffix_is_recognized,
    errno_tag, exact_source_write_refusal, get_last_error_tag, is_source_input_replay_record,
    operand_free_unknown_descriptor_operation_tag, receipted_output_entries,
    source_input_replay_prefix_end, unknown_descriptor_bad_descriptor_failure_tag,
    unknown_descriptor_get_osfhandle_tag, unknown_descriptor_open_at_tag,
    unknown_descriptor_read_dir_tag, unknown_descriptor_read_file_metadata_tag,
    unknown_descriptor_read_operation_tag, unknown_descriptor_set_file_times_tag,
    unknown_descriptor_unlink_at_tag, unknown_descriptor_write_operation_tag,
    unknown_descriptor_write_payload_operation_tag, unknown_native_handle_close_tag,
    unknown_native_handle_final_path_tag, unknown_native_handle_mutation_tag,
};

pub use selection::{
    SelectedCompilerProgramEntry, SelectedProgramEntry, SelectedProgramEntryCallingPlans,
    select_compiler_program_entry, selected_program_entry_machine,
    validate_selected_program_entry_calling_plan, validate_selected_program_entry_shape,
};

pub use selection::root_bindings::RootBinding;
use selection::root_bindings::collect_root_bindings;

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use vocabulary::{
    TargetBuildVocabulary, build_reaches_filesystem_facet, has_exact_toolchain_build_facet,
    target_build_vocabulary, validate_immutable_build_target,
};

pub use vocabulary::is_build_machine;

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

struct SelectedAdmittedBuildMachine {
    entry: PreparedBuildMachineEntry,
    symbol: SymbolHandle,
    name: String,
    normalized_callable_identity: String,
    optimization_admission: optimization::BuildOptimizationAdmission,
    target_vocabulary: Option<TargetBuildVocabulary>,
    filesystem_reachable: bool,
    execution_mode: BuildMachineExecutionMode,
    initial_build: BuildTimeValue,
}

enum AdmittedBuildMachine {
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
    prepared: PreparedBuildMachineProgram,
    machine: AdmittedBuildMachine,
    operational_plan: flow_effects::OperationalPlan,
    service_reach_plan: flow_effects::ServiceReachInferencePlan,
    filesystem_scope: BuildMachineFilesystemScope,
    evaluation_sponsor: Option<BuildEvaluationSponsor>,
    selected_target_profile: Option<target::TargetProfile>,
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

pub fn reject_uncompiled_generated_sources(
    computed: &ComputedBuildConfig,
) -> Result<(), Vec<Diagnostic>> {
    let Some(first) = computed.generated_sources.first() else {
        return Ok(());
    };
    let digest = first.digest();
    Err(vec![Diagnostic::error(format!(
        "build handed off {} captured generated source(s), beginning with `{}` ({} bytes, sha256 {:02x}{:02x}{:02x}{:02x}), but the frozen final compilation pass is not implemented yet",
        computed.generated_sources.len(),
        String::from_utf8_lossy(first.relative_path()),
        first.bytes().len(),
        digest[0],
        digest[1],
        digest[2],
        digest[3],
    ))])
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
    })
}

/// Consume one admitted build activation and extract its durable configuration
/// and evaluation evidence.
pub fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let AdmittedBuildProgram {
        prepared,
        machine,
        operational_plan: _,
        service_reach_plan: _,
        filesystem_scope,
        evaluation_sponsor,
        selected_target_profile,
    } = admitted;
    let AdmittedBuildMachine::Selected(selected) = machine else {
        return Ok(ComputedBuildConfig {
            config: BuildConfig::default(),
            optimization_report_request: optimization_core::OptimizationReportRequest::Suppressed,
            evaluation_usage: None,
            observation_summary: None,
            selected_build_machine_symbol: None,
            generated_sources: Vec::new(),
        });
    };
    let SelectedAdmittedBuildMachine {
        entry: machine_entry,
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity: _,
        optimization_admission,
        target_vocabulary,
        filesystem_reachable,
        execution_mode,
        initial_build,
    } = selected;
    let typed = prepared.typed();
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("admitted build entry remains in its owned prepared program");
    let initial_arguments = vec![initial_build];
    let evaluation_sponsor = evaluation_sponsor.as_ref();
    let measured = match evaluation_sponsor {
        Some(sponsor) => {
            build_time_evaluation::evaluate_build_machine_entry_arguments_measured_with_sponsor(
                &prepared,
                &machine_entry,
                initial_arguments.clone(),
                execution_mode.clone(),
                sponsor,
            )
        }
        None => build_time_evaluation::evaluate_build_machine_entry_arguments_measured(
            &prepared,
            &machine_entry,
            initial_arguments.clone(),
            execution_mode,
        ),
    }
    .map_err(|reason| {
        let partial_evidence = reason
            .observations()
            .filter(|observations| !observations.filesystem_operation_attempts().is_empty())
            .map(|observations| {
                let attempts = observations.filesystem_operation_attempts();
                let halted = attempts
                    .iter()
                    .filter(|attempt| {
                        matches!(
                            attempt.outcome(),
                            Some(checked_interpreter::FilesystemOperationAttemptOutcome::EvaluationHalted(_))
                        )
                    })
                    .count();
                let grant_refusals = attempts
                    .iter()
                    .map(|attempt| attempt.grant_refusals().len())
                    .sum::<usize>();
                let scalar_operands = attempts
                    .iter()
                    .map(|attempt| attempt.scalar_operands().len())
                    .sum::<usize>();
                let byte_operands = attempts
                    .iter()
                    .map(|attempt| attempt.byte_operands().len())
                    .sum::<usize>();
                let path_like_operands = attempts
                    .iter()
                    .map(|attempt| attempt.path_like_operands().len())
                    .sum::<usize>();
                let logical_handle_operands = attempts
                    .iter()
                    .map(|attempt| attempt.logical_handle_inputs().len())
                    .sum::<usize>();
                let mutable_carrier_operands = attempts
                    .iter()
                    .map(|attempt| {
                        attempt.mutable_byte_operand_resolutions().len()
                            + attempt.mutable_i64_operand_resolutions().len()
                    })
                    .sum::<usize>();
                let rooted_path_operands = attempts
                    .iter()
                    .map(|attempt| attempt.rooted_path_operand_resolutions().len())
                    .sum::<usize>();
                format!(
                    "; partial non-admission filesystem evidence: {} call(s), {halted} evaluator-halted, {grant_refusals} grant refusal(s), {scalar_operands} scalar operand(s), {byte_operands} immutable byte operand(s), {path_like_operands} path-like operand(s), {rooted_path_operands} rooted-path operand(s), {logical_handle_operands} logical-handle operand(s), {mutable_carrier_operands} mutable-carrier operand(s)",
                    attempts.len()
                )
            })
            .unwrap_or_default();
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}{partial_evidence}"
        ))]
    })?;
    let usage = measured.usage();
    let replay = if filesystem_reachable {
        let attempts = measured.observations().filesystem_operation_attempts();
        if let Some(operation_suffix_start) =
            source_input_replay_prefix_end(attempts).filter(|end| *end < attempts.len())
        {
            if operation_suffix_start == 0
                && attempts.len() == 1
                && exact_source_write_refusal(&attempts[0])
            {
                checked_interpreter::FilesystemReplay::from_source_write_refusal_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 2
                && unknown_descriptor_bad_descriptor_failure_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
                && errno_tag(attempts[operation_suffix_start + 1].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_failure_with_errno_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && operand_free_unknown_descriptor_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_operation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && attempts[operation_suffix_start].operation_tag() == 10
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_seek_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_open_at_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_open_at_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_unlink_at_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_dir_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_write_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_set_file_times_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_write_payload_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_file_metadata_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_get_osfhandle_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_close_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_final_path_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_mutation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_mutation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 2
                && (unknown_native_handle_close_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ) || unknown_native_handle_final_path_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ) || unknown_native_handle_mutation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ))
                && get_last_error_tag(attempts[operation_suffix_start + 1].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_failure_with_last_error_observations(
                    measured.observations(),
                )
                .ok()
            } else {
                checked_interpreter::FilesystemReplay::from_input_output_observations(
                    measured.observations(),
                )
                .ok()
            }
        } else {
            is_source_input_replay_record(measured.observations())
                .then(|| {
                    checked_interpreter::FilesystemReplay::from_source_input_observations(
                        measured.observations(),
                    )
                })
                .and_then(Result::ok)
        }
    } else {
        None
    };
    let replay_has_no_output_attempts = replay
        .as_ref()
        .is_some_and(|replay| !replay.has_output_attempts());
    let replay_includes_complete_no_output_failure = replay.is_some()
        && source_input_replay_prefix_end(measured.observations().filesystem_operation_attempts())
            .is_some_and(|suffix_start| {
                complete_no_output_failure_suffix_is_recognized(
                    measured.observations().filesystem_operation_attempts(),
                    suffix_start,
                )
            });
    let receipted_output_entries = replay.as_ref().and_then(receipted_output_entries);
    let replay_usage = if let Some(replay) = replay {
        let replay_mode = BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::ReplayFilesystem(replay),
            filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout::default(),
        };
        let replayed = match evaluation_sponsor {
            Some(sponsor) => {
                build_time_evaluation::evaluate_build_machine_entry_arguments_measured_with_sponsor(
                    &prepared,
                    &machine_entry,
                    initial_arguments,
                    replay_mode,
                    sponsor,
                )
            }
            None => build_time_evaluation::evaluate_build_machine_entry_arguments_measured(
                &prepared,
                &machine_entry,
                initial_arguments,
                replay_mode,
            ),
        }
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "build-time replay of `{machine_name}` failed: {reason}"
            ))]
        })?;
        if replayed.value() != measured.value()
            || replayed.observations() != measured.observations()
            || replayed.executed_root_bindings() != measured.executed_root_bindings()
        {
            return Err(vec![Diagnostic::error(format!(
                "build-time replay of `{machine_name}` changed its result or operation record"
            ))]);
        }
        Some(replayed.usage())
    } else {
        None
    };
    let source_inputs_replayed = replay_usage.is_some();
    let replayed_output_tree = if replay_has_no_output_attempts {
        Some(empty())
    } else {
        receipted_output_entries
            .as_ref()
            .map(|entries| {
                let mut regular_files = std::collections::BTreeMap::new();
                let mut normalized_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    let normalized = match entry {
                        ReceiptedOutputEntry::Directory { .. }
                        | ReceiptedOutputEntry::Symlink { .. } => entry.clone(),
                        ReceiptedOutputEntry::File(file) => {
                            regular_files.insert(
                                file.relative_path.clone(),
                                (file.bytes.clone(), file.executable),
                            );
                            entry.clone()
                        }
                        ReceiptedOutputEntry::HardLink {
                            existing_relative_path,
                            relative_path,
                        } => {
                            let (bytes, executable) = regular_files
                                .get(existing_relative_path)
                                .expect("validated hard link follows a regular-file name")
                                .clone();
                            regular_files
                                .insert(relative_path.clone(), (bytes.clone(), executable));
                            ReceiptedOutputEntry::File(ReceiptedOutputFile {
                                relative_path: relative_path.clone(),
                                bytes,
                                executable,
                            })
                        }
                    };
                    normalized_entries.push(normalized);
                }
                let replayed_entries = normalized_entries
                    .iter()
                    .map(|entry| match entry {
                        ReceiptedOutputEntry::Directory { relative_path } => {
                            ReplayedBuildOutputEntry::directory(relative_path)
                        }
                        ReceiptedOutputEntry::File(file) => ReplayedBuildOutputEntry::regular_file(
                            &file.relative_path,
                            &file.bytes,
                            file.executable,
                        ),
                        ReceiptedOutputEntry::Symlink {
                            relative_path,
                            target_spelling,
                        } => {
                            ReplayedBuildOutputEntry::symbolic_link(relative_path, target_spelling)
                        }
                        ReceiptedOutputEntry::HardLink { .. } => {
                            unreachable!("hard links are normalized before staged commitment")
                        }
                    })
                    .collect::<Vec<_>>();
                replayed_output_tree(&replayed_entries)
            })
            .transpose()?
    };
    let observation_ceiling = if filesystem_reachable {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    let filesystem_operation_schema_version = measured
        .observations()
        .filesystem_operation_schema_version();
    let filesystem_operation_attempts = measured
        .observations()
        .filesystem_operation_attempts()
        .iter()
        .map(|attempt| {
            let authorized_paths = attempt
                .authorized_paths()
                .iter()
                .map(|path| {
                    let root = if path.root() == BUILD_SOURCE_ROOT_IDENTITY {
                        BuildFilesystemRoot::Source
                    } else if path.root() == BUILD_OUTPUT_ROOT_IDENTITY {
                        BuildFilesystemRoot::Output
                    } else {
                        return Err(Diagnostic::error(format!(
                            "build-time evaluation of `{machine_name}` returned unknown filesystem grant-root identity `{}`",
                            path.root().get()
                        )));
                    };
                    Ok(BuildFilesystemAuthorizedPath {
                        operand_ordinal: path.operand_ordinal(),
                        access: match path.access() {
                            checked_interpreter::FilesystemGrantAccess::Read => {
                                BuildFilesystemGrantAccess::Read
                            }
                            checked_interpreter::FilesystemGrantAccess::Write => {
                                BuildFilesystemGrantAccess::Write
                            }
                        },
                        root,
                        relative_path: path.relative_path().to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let logical_handle_inputs = attempt
                .logical_handle_inputs()
                .iter()
                .map(|input| BuildFilesystemLogicalHandleInput {
                    operand_ordinal: input.operand_ordinal(),
                    kind: project_logical_handle_kind(input.kind()),
                    resolution: project_logical_handle_input_resolution(input.resolution()),
                })
                .collect();
            let scalar_operands = attempt
                .scalar_operands()
                .iter()
                .map(|operand| BuildFilesystemScalarOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    value: project_scalar_operand_value(operand.value()),
                })
                .collect();
            let byte_operands = attempt
                .byte_operands()
                .iter()
                .map(|operand| BuildFilesystemByteOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let path_like_operands = attempt
                .path_like_operands()
                .iter()
                .map(|operand| BuildFilesystemPathLikeOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let rooted_path_operand_resolutions = attempt
                .rooted_path_operand_resolutions()
                .iter()
                .map(|operand| {
                    let root = if operand.root() == BUILD_SOURCE_ROOT_IDENTITY {
                        BuildFilesystemRoot::Source
                    } else if operand.root() == BUILD_OUTPUT_ROOT_IDENTITY {
                        BuildFilesystemRoot::Output
                    } else {
                        return Err(Diagnostic::error(format!(
                            "build-time evaluation of `{machine_name}` returned unknown rooted-path operand identity `{}`",
                            operand.root().get()
                        )));
                    };
                    Ok(BuildFilesystemRootedPathOperandResolution {
                        operand_ordinal: operand.operand_ordinal(),
                        root,
                        relative_path: operand.relative_path().to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let returned_paths = attempt
                .returned_paths()
                .iter()
                .map(|returned| BuildFilesystemReturnedPath {
                    operand_ordinal: returned.operand_ordinal(),
                    kind: match returned.kind() {
                        checked_interpreter::FilesystemReturnedPathKind::ReadLinkPayload => {
                            BuildFilesystemReturnedPathKind::ReadLinkPayload
                        }
                        checked_interpreter::FilesystemReturnedPathKind::CanonicalPath => {
                            BuildFilesystemReturnedPathKind::CanonicalPath
                        }
                        checked_interpreter::FilesystemReturnedPathKind::FinalPath => {
                            BuildFilesystemReturnedPathKind::FinalPath
                        }
                    },
                    completeness: match returned.completeness() {
                        checked_interpreter::FilesystemReturnedPathCompleteness::Complete => {
                            BuildFilesystemReturnedPathCompleteness::Complete
                        }
                        checked_interpreter::FilesystemReturnedPathCompleteness::LimitReached => {
                            BuildFilesystemReturnedPathCompleteness::LimitReached
                        }
                    },
                    bytes: returned.bytes().to_vec(),
                })
                .collect();
            let observed_byte_regions = attempt
                .observed_byte_regions()
                .iter()
                .map(|region| {
                    Ok(BuildFilesystemObservedByteRegion {
                        output_operand_ordinal: region.output_operand_ordinal(),
                        kind: match region.kind() {
                            checked_interpreter::FilesystemObservedByteRegionKind::SequentialFileRead => {
                                BuildFilesystemObservedByteRegionKind::SequentialFileRead
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::PositionedFileRead => {
                                BuildFilesystemObservedByteRegionKind::PositionedFileRead
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::DirectoryRecords => {
                                BuildFilesystemObservedByteRegionKind::DirectoryRecords
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::FindEntry => {
                                BuildFilesystemObservedByteRegionKind::FindEntry
                            }
                        },
                        offset: u64::try_from(region.offset()).map_err(|_| {
                            Diagnostic::error(
                                "build observation byte-region offset is not canonically representable",
                            )
                        })?,
                        length: u64::try_from(region.length()).map_err(|_| {
                            Diagnostic::error(
                                "build observation byte-region length is not canonically representable",
                            )
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let metadata_observations = attempt
                .metadata_observations()
                .iter()
                .map(|observation| BuildFilesystemMetadataObservation {
                    output_operand_ordinal: observation.output_operand_ordinal(),
                    kind: match observation.kind() {
                        checked_interpreter::FilesystemMetadataObservationKind::FollowedPath => {
                            BuildFilesystemMetadataObservationKind::FollowedPath
                        }
                        checked_interpreter::FilesystemMetadataObservationKind::OpenDescriptor => {
                            BuildFilesystemMetadataObservationKind::OpenDescriptor
                        }
                        checked_interpreter::FilesystemMetadataObservationKind::UnfollowedFinalPath => {
                            BuildFilesystemMetadataObservationKind::UnfollowedFinalPath
                        }
                    },
                    device: observation.device(),
                    mode: observation.mode(),
                    link_count: observation.link_count(),
                    inode: observation.inode(),
                    user: observation.user(),
                    group: observation.group(),
                    referenced_device: observation.referenced_device(),
                    access_time: observation.access_time(),
                    modification_time: observation.modification_time(),
                    change_time: observation.change_time(),
                    birth_time: observation.birth_time(),
                    size: observation.size(),
                    blocks_512: observation.blocks_512(),
                    preferred_block_size: observation.preferred_block_size(),
                })
                .collect();
            let mutable_byte_operand_resolutions = attempt
                .mutable_byte_operand_resolutions()
                .iter()
                .map(|operand| BuildFilesystemMutableByteOperandResolution {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let mutable_i64_operand_resolutions = attempt
                .mutable_i64_operand_resolutions()
                .iter()
                .map(|operand| BuildFilesystemMutableI64OperandResolution {
                    operand_ordinal: operand.operand_ordinal(),
                    value: operand.value(),
                })
                .collect();
            let mutable_byte_operands = attempt
                .mutable_byte_operands()
                .iter()
                .map(|operand| BuildFilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    pre_bytes: operand.pre_bytes().to_vec(),
                    post_bytes: operand.post_bytes().to_vec(),
                })
                .collect();
            let mutable_i64_operands = attempt
                .mutable_i64_operands()
                .iter()
                .map(|operand| BuildFilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal(),
                    pre_value: operand.pre_value(),
                    post_value: operand.post_value(),
                })
                .collect();
            let logical_handle_output = attempt.logical_handle_output().map(|output| {
                BuildFilesystemLogicalHandleOutput {
                    kind: project_logical_handle_kind(output.kind()),
                    identity: project_logical_handle_identity(output.identity()),
                    source: project_logical_handle_output_source(output.source()),
                }
            });
            let retired_logical_handles = attempt
                .retired_logical_handles()
                .iter()
                .copied()
                .map(project_logical_handle_identity)
                .collect();
            Ok(BuildFilesystemOperationAttempt {
                operation_tag: attempt.operation_tag(),
                provider: match attempt.provider() {
                checked_interpreter::FilesystemObservationProvider::Virtual => {
                    BuildFilesystemProvider::Virtual
                }
                checked_interpreter::FilesystemObservationProvider::RealUnscoped => {
                    BuildFilesystemProvider::RealUnscoped
                }
                checked_interpreter::FilesystemObservationProvider::RealScoped => {
                    BuildFilesystemProvider::RealScoped
                }
            },
                observation_class: if source_inputs_replayed {
                    BuildFilesystemOperationObservationClass::Receipted
                } else {
                    BuildFilesystemOperationObservationClass::Volatile
                },
                result: project_operation_result(
                    attempt
                        .result()
                        .expect("successful build evaluation cannot retain a halted filesystem call"),
                ),
                post_error: attempt
                    .post_error()
                    .expect("successful build evaluation cannot retain a halted filesystem call"),
                scalar_operands,
                byte_operands,
                path_like_operands,
                rooted_path_operand_resolutions,
                returned_paths,
                observed_byte_regions,
                metadata_observations,
                mutable_byte_operand_resolutions,
                mutable_i64_operand_resolutions,
                mutable_byte_operands,
                mutable_i64_operands,
                authorized_paths,
                logical_handle_inputs,
                logical_handle_output,
                retired_logical_handles,
                grant_refusals: attempt
                    .grant_refusals()
                    .iter()
                    .map(|refusal| BuildFilesystemGrantRefusal {
                        operand_ordinal: refusal.operand_ordinal(),
                        access: match refusal.access() {
                            checked_interpreter::FilesystemGrantAccess::Read => {
                                BuildFilesystemGrantAccess::Read
                            }
                            checked_interpreter::FilesystemGrantAccess::Write => {
                                BuildFilesystemGrantAccess::Write
                            }
                        },
                        reason: match refusal.reason() {
                            checked_interpreter::FilesystemGrantRefusalReason::Unresolvable => {
                                BuildFilesystemGrantRefusalReason::Unresolvable
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::OutsideGrantedRoots => {
                                BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::UnrepresentableRootedPath => {
                                BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => {
                                BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded
                            }
                        },
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    let included_source_handoffs = measured
        .observations()
        .build_included_sources()
        .iter()
        .map(|source| {
            if source.root() != BUILD_OUTPUT_ROOT_IDENTITY {
                return Err(Diagnostic::error(format!(
                    "build-time evaluation of `{machine_name}` handed off a generated source outside the compiler-issued Output root"
                )));
            }
            Ok(BuildIncludedSourceHandoff {
                relative_path: source.relative_path().to_vec(),
                filesystem_attempt_ordinal: u64::try_from(
                    source.filesystem_attempt_ordinal(),
                )
                .map_err(|_| {
                    Diagnostic::error(format!(
                        "build-time evaluation of `{machine_name}` produced an included-source ordinal that exceeds canonical u64"
                    ))
                })?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    let filesystem_host_observed = measured.observations().filesystem_host_observed();
    let build_log = measured.observations().build_log().to_vec();
    let root_bindings = collect_root_bindings(typed, machine, measured.executed_root_bindings())?;
    let mut arguments = measured.into_value();
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let (mut config, optimization_report) = extract_build_config(
        &augmented,
        optimization_admission,
        selected_target_profile,
        target_vocabulary.is_some(),
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` produced an invalid Build: {reason}"
        ))]
    })?;
    config.grants = harvest_root_grants(typed, machine).map_err(|diagnostic| vec![diagnostic])?;
    config.provider_selections = harvest_provider_selections(typed, machine)?;
    config.opaque_representation_selections =
        representation_planning::harvest_opaque_representation_selections(typed, machine)?;
    config.wire_compatibility_demands = harvest_wire_compatibility_demands(typed, machine)?;
    config.root_bindings = root_bindings;
    let captured_output_tree = filesystem_scope.staged_output_tree(filesystem_reachable)?;
    let (staged_output_tree, complete_replay_verified) = match (
        replayed_output_tree,
        captured_output_tree,
        filesystem_scope.is_replay(),
    ) {
        (Some(replayed), Some(captured), false) => {
            if replayed != captured {
                return Err(vec![Diagnostic::error(format!(
                    "build-time replay of `{machine_name}` reproduced an Output tree that differs from sponsored staged-output custody"
                ))]);
            }
            (Some(captured), true)
        }
        (Some(replayed), None, true) => (Some(replayed), true),
        (Some(replayed), None, false) if replay_includes_complete_no_output_failure => {
            (Some(replayed), true)
        }
        (Some(_), None, false) => (None, false),
        (Some(_), Some(_), true) => {
            unreachable!("replay scope cannot capture a physical staged-output tree")
        }
        (None, captured, _) => (captured, false),
    };
    if complete_replay_verified && !source_inputs_replayed {
        return Err(vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` completed without exact source-input replay"
        ))]);
    }
    if complete_replay_verified && staged_output_tree.is_none() {
        return Err(vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` completed without staged-output custody"
        ))]);
    }
    let filesystem_replay_verdict =
        BuildFilesystemReplayVerdict::new(if complete_replay_verified {
            BuildFilesystemReplayDisposition::Complete
        } else if source_inputs_replayed {
            BuildFilesystemReplayDisposition::SourceInputsOnly
        } else {
            BuildFilesystemReplayDisposition::NotReplayed
        });
    let realized_observation = if filesystem_replay_verdict.is_complete() {
        BuildObservationClass::Receipted
    } else if filesystem_host_observed {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    if realized_observation > observation_ceiling {
        return Err(vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` observed filesystem host state outside its static observation ceiling"
        ))]);
    }
    let generated_sources = match staged_output_tree.as_ref() {
        Some(tree) => {
            let included_source_paths = included_source_handoffs
                .iter()
                .map(|handoff| handoff.relative_path.clone())
                .collect::<Vec<_>>();
            select_included_sources(tree, &included_source_paths)?
        }
        None if included_source_handoffs.is_empty() => Vec::new(),
        None => {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` handed off generated source without sponsored staged-output custody"
            ))]);
        }
    };
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(BuildEvaluationUsage {
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            invocation_fuel_ceiling: usage.fuel_ceiling(),
            sponsor_schema_version: evaluation_sponsor
                .map(|sponsor| sponsor.limits().schema_version()),
            session_fuel_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_fuel_units()),
            session_build_log_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_build_log_bytes()),
            session_filesystem_attempt_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_filesystem_operation_attempts()),
            session_live_filesystem_handle_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_filesystem_handles()),
            session_live_cell_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_cells()),
            session_live_text_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
            session_result_cell_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_result_cells()),
            session_result_text_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
            session_peak_live_filesystem_handles: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
            session_peak_live_cells: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_cells),
            session_peak_live_text_bytes: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
            fuel_units: usage.fuel_units(),
            replay_fuel_units: replay_usage.map_or(0, |usage| usage.fuel_units()),
            build_log_bytes: usage.build_log_bytes(),
            replay_build_log_bytes: replay_usage.map_or(0, |usage| usage.build_log_bytes()),
            filesystem_operation_attempts: usage.filesystem_operation_attempts(),
            replay_filesystem_operation_attempts: replay_usage
                .map_or(0, |usage| usage.filesystem_operation_attempts()),
            peak_live_cells: usage.peak_live_cells(),
            replay_peak_live_cells: replay_usage.map_or(0, |usage| usage.peak_live_cells()),
            peak_live_text_bytes: usage.peak_live_text_bytes(),
            replay_peak_live_text_bytes: replay_usage
                .map_or(0, |usage| usage.peak_live_text_bytes()),
            result_cells: usage.result_cells(),
            replay_result_cells: replay_usage.map_or(0, |usage| usage.result_cells()),
            result_text_bytes: usage.result_text_bytes(),
            replay_result_text_bytes: replay_usage.map_or(0, |usage| usage.result_text_bytes()),
        }),
        observation_summary: Some(BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            ceiling: observation_ceiling,
            realized: realized_observation,
            filesystem_operation_schema_version,
            filesystem_operation_attempts,
            canonical_source_metadata_identity: filesystem_scope
                .canonical_source_metadata_identity(),
            filesystem_replay_verdict,
            included_source_handoffs,
            staged_output_tree,
            build_log,
        }),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AdmittedBuildAuthorityVerdict, AdmittedBuildProgramDisposition, BuildConfig,
        BuildMachineFilesystemScope, admit_build_program,
    };
    use std::path::PathBuf;

    #[test]
    fn admitted_no_build_checkpoint_preserves_default_configuration_and_evidence() {
        let typed = typed_trees::TypedTrees::default();
        let scope = BuildMachineFilesystemScope::for_root(
            std::path::Path::new("main.omg"),
            PathBuf::from("build"),
            None,
        );
        let admitted = admit_build_program(&typed, None, &scope, None, None)
            .expect("the empty program has an explicit no-build disposition");

        assert_eq!(
            admitted.disposition(),
            AdmittedBuildProgramDisposition::NoBuildMachine
        );
        assert_eq!(
            admitted.authority_verdict(),
            AdmittedBuildAuthorityVerdict::NoBuildMachine
        );
        assert_eq!(admitted.selected_build_machine_symbol(), None);
        assert_eq!(admitted.selected_build_machine_callable_identity(), None);
        assert_eq!(admitted.initial_build_snapshot(), None);
        assert!(admitted.operational_plan().machines().is_empty());
        assert!(admitted.service_reach_plan().machines().is_empty());

        let executed = admitted.execute().expect("consume no-build checkpoint");
        assert_eq!(executed.config, BuildConfig::default());
        assert_eq!(
            executed.optimization_report_request,
            optimization_core::OptimizationReportRequest::Suppressed
        );
        assert_eq!(executed.evaluation_usage, None);
        assert_eq!(executed.observation_summary, None);
        assert_eq!(executed.selected_build_machine_symbol, None);
        assert!(executed.generated_sources.is_empty());
    }
}
