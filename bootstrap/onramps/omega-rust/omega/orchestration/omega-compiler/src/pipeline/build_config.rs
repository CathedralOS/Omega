//! THE BUILD CONFIG (build_and_package_model.md + its 2026-07-04 addendum):
//! image facts come from `build.omg`'s augmenting machine, never from an
//! invented config grammar. When the program (build.omg is ordinary source,
//! auto-included next to main.omg) defines the conventionally-named free
//! machine `build(build: &mut Build)`, the compiler evaluates it at build time
//! (purity-gated, the L0 engine) with a ZII `Build` and reads the augmented
//! value back:
//!
//! ```omega
//! data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
//! data Build { subsystem: Subsystem; freestanding: bool; }
//! machine build(build: &mut Build) {
//!     build.subsystem = Subsystem::EfiApplication;
//!     build.freestanding = true;
//! }
//! ```
//!
//! - `subsystem` is loader METADATA (a PE header u16 the compiler copies; it
//!   does not select the emitter). The ZII zero case is `Console` -- the
//!   correct default falls out of the type. `Unspecified(value)` is the
//!   escape hatch: any loader value a platform invents, with no compiler
//!   release.
//! - `freestanding` ("trust no host packages" -> the empty host-ABI plan) is
//!   stated as itself -- previously fused into the `efi_application` name.
//! - Absent build.omg == an empty `build` machine == the zero `Build`: the
//!   hosted console default.
//! - `builder.roots.bind(target::ProgramEntry, Exact::machine);` is a static
//!   declaration harvested from the same authoritative build machine. It
//!   selects the exact source entry and performs no name-based discovery.

use psi_build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemGrants,
    BuildTimeValue, PreparedBuildMachineProgram,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use std::path::{Path, PathBuf};

const BUILD_MACHINE: &str = "build";

/// Host filesystem authority granted to an admitted `build.omg` machine.
///
/// Source roots are read-only. The build directory is the only write root and
/// also permits read-back through the checked interpreter's `RealScoped`
/// contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildMachineFilesystemScope {
    source_root: PathBuf,
    build_dir: PathBuf,
}

impl BuildMachineFilesystemScope {
    pub(crate) fn for_root(root_path: &Path, build_dir: PathBuf) -> Self {
        let source_root = root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            source_root,
            build_dir,
        }
    }

    fn grants(&self) -> BuildMachineFilesystemGrants {
        BuildMachineFilesystemGrants {
            read_roots: vec![self.source_root.clone()],
            write_roots: vec![self.build_dir.clone()],
        }
    }

    fn ensure_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        std::fs::create_dir_all(&self.build_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to create build machine filesystem write root `{}`: {error}",
                self.build_dir.display()
            ))]
        })
    }
}

/// The image facts the pipeline consumes, extracted from the augmented
/// `Build`. ZII: the default IS the zero value's meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    /// PE optional-header Subsystem word (console 3 when unstated).
    pub subsystem: u16,
    /// Freestanding image: empty host-ABI plan, no import thunks.
    pub freestanding: bool,
    /// CH10 ROOT GRANTS (GR3): the symbol paths the final build accepted
    /// via `b.accept_boundary<pkg::symbol>();` -- harvested STATICALLY
    /// from the build machine's marker calls (grants are declarations,
    /// not runtime effects; the evaluator serves the marker as a no-op).
    pub grants: Vec<String>,
    /// PRV4c: explicit provider-type choices for boundary slots. These are
    /// declarations harvested from the authoritative build machine; they are
    /// validated against derived candidates before selection grants anything.
    pub provider_selections: Vec<ProviderSelection>,
    /// Chapter 21 channel/store compatibility demands. Each marker names the
    /// edge, format lineage, local and peer schemas, and the directional facts
    /// the final build requires.
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    /// Target-owned inbound root slots bound by the authoritative build
    /// machine. The binding names an exact source machine; no entry discovery
    /// or naming convention participates once a binding is present.
    pub root_bindings: Vec<RootBinding>,
}

/// Accounting-only projection of the transitional typed-tree build evaluator.
/// This is not terminal-Psi fuel and does not participate in `BuildConfig` or
/// program identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationUsage {
    pub usage_schema_version: u32,
    pub step_schedule_marker: u32,
    pub fuel_units: u64,
    pub result_cells: u64,
}

pub const BUILD_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Normalized build-host observation class for one selected build machine.
///
/// The current compiler has no receipted build-host provider. Its scoped real
/// filesystem provider is therefore conservatively volatile; console output
/// and pure configuration do not observe external input and remain hermetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildObservationClass {
    Hermetic,
    Receipted,
    Volatile,
}

/// Compiler-issued observation facts for one completed build-machine run.
///
/// This is execution evidence, not capability/API comparison identity. A
/// volatile row carries no replay receipt and makes no rebuildability claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildObservationSummary {
    schema_version: u32,
    ceiling: BuildObservationClass,
    realized: BuildObservationClass,
}

impl BuildObservationSummary {
    pub const fn schema_version(self) -> u32 {
        self.schema_version
    }

    pub const fn ceiling(self) -> BuildObservationClass {
        self.ceiling
    }

    pub const fn realized(self) -> BuildObservationClass {
        self.realized
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComputedBuildConfig {
    pub config: BuildConfig,
    pub evaluation_usage: Option<BuildEvaluationUsage>,
    pub observation_summary: Option<BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<psi_symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    pub slot: omega_target::ProgramEntrySlotDeclaration,
}

/// Resolve the selected target's `ProgramEntry` binding. This is the first
/// implemented target-root slot; other root-slot kinds reject rather than
/// being accepted and then ignored. A build file may describe a target matrix:
/// well-formed slots owned by other known profiles are validated and left for
/// those profiles, while this selection consumes only the chosen profile's
/// exact row. With no root declarations at all the caller may still enter the
/// explicit migration fallback for the remaining corpus.
pub(crate) fn selected_program_entry_machine<'config>(
    config: &'config BuildConfig,
    target_name: Option<&str>,
) -> Result<Option<SelectedProgramEntry<'config>>, Vec<Diagnostic>> {
    if config.root_bindings.is_empty() {
        return Ok(None);
    }

    let selected_profile = omega_target::TargetProfile::from_omega_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    let mut diagnostics = Vec::new();
    let mut selected_bindings = Vec::new();
    for binding in &config.root_bindings {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match omega_target::TargetProfile::from_root_slot_owner(profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.target_name(),
                binding.slot
            )));
            continue;
        };
        if profile != selected_profile {
            continue;
        }
        selected_bindings.push((required_slot, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in selected_profile.required_root_slots() {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let binding = match matches.as_slice() {
            [(_, binding)] => *binding,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has no bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has more than one bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
        };
        let Some(program_entry) = required_slot.program_entry() else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}::{}` uses a target-required schema that the ProgramEntry source-lowering path does not implement",
                required_slot.owner().root_slot_owner_name(),
                required_slot.slot_name()
            )));
            continue;
        };
        program_entries.push((program_entry, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(slot, binding)] => Ok(Some(SelectedProgramEntry {
            machine_name: binding.implementation.as_str(),
            slot: *slot,
        })),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no supported ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` declares more than one ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
    }
}

/// Validate the source half of the currently implemented `ProgramEntry`
/// schema. Hosted targets expose no arrival parameters: the selected machine
/// is either free or has exactly one mutable `self` receiver for later bridge
/// provisioning. Freestanding parameters must exactly match the canonical
/// typed positions on the target-selected arrival requirement.
pub(crate) fn validate_selected_program_entry_shape(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
) -> Result<super::SelectedProgramEntrySourceSignature, Vec<Diagnostic>> {
    let machine_name = selected.machine_name;
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return Err(vec![Diagnostic::error(format!(
            "build root slot names unknown entry machine `{machine_name}`"
        ))]);
    };
    let Some(entry) = typed.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "entry machine `{machine_name}` has no executable entry state"
        ))]);
    };

    let mut diagnostics = Vec::new();
    if !typed.machine_type_parameters(machine).is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` is generic; a root slot must bind one exact machine"
        )));
    }
    if entry.return_type.is_valid() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` returns a value, but `ProgramEntry` has no result"
        )));
    }

    let parameters = typed.state_parameters(entry);
    let self_parameters = parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    if self_parameters.len() > 1 {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` has more than one `self` receiver"
        )));
    }
    if let Some(receiver) = self_parameters.first()
        && !receiver.is_mutable
    {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` has a receiver, but `ProgramEntry` provisions it as an exclusive `&mut self` loan"
        )));
    }
    if !self_parameters.is_empty()
        && let Some(attached_data) = machine.attached_data.as_ref()
        && let Some(definition) = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == attached_data.as_str())
        && psi_typed_trees_to_checked_trees::data_requires_establishment(typed, definition)
    {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` requests a provisioned `{}` receiver, but its all-zero image is not a valid value; use a free entry and construct the state explicitly",
            attached_data.as_str()
        )));
    }

    let visible = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    match selected.slot.visible_parameters {
        omega_target::ProgramEntryVisibleParameters::None if !visible.is_empty() => {
            diagnostics.push(Diagnostic::error(format!(
                "hosted `ProgramEntry` exposes no arrival parameters, but `{machine_name}` declares `{}`",
                visible
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>()
                    .join("`, `")
            )));
        }
        omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            if visible.len() != 2 =>
        {
            diagnostics.push(Diagnostic::error(format!(
                "target schema `{:?}` exposes exactly image and initial-storage roots, but `{machine_name}` declares {} visible parameter{}",
                selected.slot.schema,
                visible.len(),
                if visible.len() == 1 { "" } else { "s" },
            )));
        }
        _ => {}
    }

    if selected.slot.visible_parameters
        == omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        && visible.len() == 2
    {
        match arrival_requirement_contract(typed, selected.slot.semantic_arrival_requirement) {
            Ok(required) if required.parameters.len() == visible.len() => {
                for (index, (actual, required)) in
                    visible.iter().zip(required.parameters.iter()).enumerate()
                {
                    if typed.normalized_type_identity(actual.type_reference) != required.identity
                        || actual.is_const != required.is_const
                        || actual.is_mutable != required.is_mutable
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "target root slot `{}::{}` requires visible parameter {index} ({}) to have exact type `{}`, but entry machine `{machine_name}` declares `{}`",
                            selected.slot.owner.root_slot_owner_name(),
                            selected.slot.slot_name,
                            ["image", "initial storage"][index],
                            required.display,
                            typed.display_type_reference_with_constraints(actual.type_reference),
                        )));
                    }
                }
            }
            Ok(required) => diagnostics.push(Diagnostic::error(format!(
                "target root slot `{}::{}` selects arrival requirement `{}` with {} visible parameters, but its target schema declares {}",
                selected.slot.owner.root_slot_owner_name(),
                selected.slot.slot_name,
                selected.slot.semantic_arrival_requirement,
                required.parameters.len(),
                visible.len(),
            ))),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let receiver = self_parameters.first().map_or(
        super::ProgramEntrySourceReceiverSignature::Free,
        |receiver| super::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            normalized_type_identity: typed
                .normalized_type_identity(receiver.type_reference)
                .into_string(),
        },
    );
    let visible_parameters = visible
        .iter()
        .enumerate()
        .map(|(index, parameter)| -> Result<_, Diagnostic> {
            let role = match index {
                0 => super::ProgramStorageEntryRootRole::Image,
                1 => super::ProgramStorageEntryRootRole::InitialStorage,
                _ => unreachable!("selected source shape validation fixed visible arity"),
            };
            let extent_value_layout =
                super::calling_policy_plans::selected_program_storage_source_extent_value_layout(
                    typed,
                    selected.slot,
                    parameter.type_reference,
                )
            .map_err(|diagnostic| {
                Diagnostic::error(format!(
                    "selected entry machine `{machine_name}` visible parameter {index} has no exact Extent value layout: {diagnostic}"
                ))
            })?;
            let value_shape = extent_value_layout.shape();
            Ok(super::SelectedProgramEntrySourceSignature::visible_parameter(
                role,
                index,
                typed
                    .normalized_type_identity(parameter.type_reference)
                    .into_string(),
                value_shape,
                extent_value_layout,
                parameter.is_const,
                parameter.is_mutable,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    super::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        selected.slot,
        machine.symbol,
        entry.symbol,
        machine.name.as_str().to_owned(),
        entry.name.as_str().to_owned(),
        typed
            .normalized_machine_overload_identity(machine)
            .expect("selected entry has one checked executable state")
            .identity(),
        receiver,
        visible_parameters,
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])
}

pub(crate) struct SelectedProgramEntryCallingPlans {
    pub(crate) semantic_boundary_entry_plan: omega_calling_conventions::BoundaryEntryPlan,
    pub(crate) storage_entry: super::program_storage_entry::SelectedProgramStorageEntryPlan,
}

pub(crate) fn validate_selected_program_entry_calling_plan(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
    realizations: &[super::calling_policy_plans::BoundaryCallingPlanRealization],
) -> Result<Option<SelectedProgramEntryCallingPlans>, Vec<Diagnostic>> {
    let (
        Some(schema_name),
        Some(physical_requirement),
        Some(physical_convention),
        Some(semantic_convention),
    ) = (
        selected.slot.boundary_schema,
        selected.slot.physical_arrival_requirement,
        selected.slot.physical_calling_convention,
        selected.slot.semantic_calling_convention,
    )
    else {
        if selected.slot.boundary_schema.is_some()
            || selected.slot.physical_arrival_requirement.is_some()
            || selected.slot.physical_calling_convention.is_some()
            || selected.slot.semantic_calling_convention.is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "target root slot `{}::{}` has an incomplete two-surface entry declaration",
                selected.slot.owner.root_slot_owner_name(),
                selected.slot.slot_name,
            ))]);
        }
        return Ok(None);
    };
    let schemas = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && definition.name.as_str() == schema_name)
        .collect::<Vec<_>>();
    let [schema] = schemas.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target root slot `{}::{}` requires exactly one loaded `{schema_name}` boundary schema, but found {}",
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            schemas.len(),
        ))]);
    };
    let semantic = arrival_requirement_contract(typed, selected.slot.semantic_arrival_requirement)
        .map_err(|diagnostic| vec![diagnostic])?;
    let physical = arrival_requirement_contract(typed, physical_requirement)
        .map_err(|diagnostic| vec![diagnostic])?;
    if semantic.signature == physical.signature
        || semantic.requirement_identity == physical.requirement_identity
    {
        return Err(vec![Diagnostic::error(format!(
            "target root slot `{}::{}` conflates physical requirement `{physical_requirement}` with semantic requirement `{}`",
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            selected.slot.semantic_arrival_requirement,
        ))]);
    }
    let semantic_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema.symbol
                && realization.requirement_machine == semantic.signature
        })
        .collect::<Vec<_>>();
    let [semantic_realization] = semantic_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for semantic requirement `{}` instead of exactly one",
            semantic_matching.len(),
            selected.slot.semantic_arrival_requirement,
        ))]);
    };
    let physical_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema.symbol
                && realization.requirement_machine == physical.signature
        })
        .collect::<Vec<_>>();
    let [physical_realization] = physical_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for physical requirement `{physical_requirement}` instead of exactly one",
            physical_matching.len(),
        ))]);
    };
    let expected_physical = match physical_convention {
        omega_target::ProgramEntryCallingConvention::MicrosoftX64 => {
            omega_calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if physical_realization.boundary_entry_plan.call.policy != expected_physical {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates physical requirement `{physical_requirement}` with {:?}, but `{}::{}` requires {:?}",
            physical_realization.boundary_entry_plan.call.policy,
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            expected_physical,
        ))]);
    }
    let expected_semantic = match semantic_convention {
        omega_target::ProgramEntryCallingConvention::MicrosoftX64 => {
            omega_calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if semantic_realization.boundary_entry_plan.call.policy != expected_semantic {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates semantic requirement `{}` with {:?}, but `{}::{}` requires {:?}",
            selected.slot.semantic_arrival_requirement,
            semantic_realization.boundary_entry_plan.call.policy,
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            expected_semantic,
        ))]);
    }
    let physical_source =
        target_owned_physical_contract_source(typed, selected.slot, schema.symbol, &physical)
            .map_err(|diagnostic| vec![diagnostic])?;
    let service_schema = omega_effects::provider_plan::ServiceSchema::from_typed(typed, schema)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "target entry schema `{schema_name}` is not a boundary service schema"
            ))]
        })?;
    let storage_entry =
        super::program_storage_entry::SelectedProgramStorageEntryPlan::from_target_slot(
            selected.slot,
            service_schema,
            semantic.requirement_identity,
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    let result_type_identity = physical.result_type_identity.ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "physical entry requirement `{physical_requirement}` has no result"
        ))]
    })?;
    let physical_contract = super::ProgramEntryPhysicalContractPlan::new(
        selected.slot,
        physical.requirement_identity,
        physical_source.package,
        physical_source.package_fingerprint,
        physical
            .parameters
            .into_iter()
            .map(|parameter| parameter.identity.into_string())
            .collect(),
        result_type_identity.into_string(),
        physical_realization.fingerprint,
        physical_realization.boundary_entry_plan.clone(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    let storage_entry = storage_entry
        .with_physical_contract(physical_contract.clone())
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    Ok(Some(SelectedProgramEntryCallingPlans {
        semantic_boundary_entry_plan: semantic_realization.boundary_entry_plan.clone(),
        storage_entry,
    }))
}

struct TargetOwnedPhysicalContractSource {
    package: omega_target::ProgramEntryPhysicalContractPackage,
    package_fingerprint: u64,
}

fn target_owned_physical_contract_source(
    typed: &TypedTrees,
    slot: omega_target::ProgramEntrySlotDeclaration,
    schema: psi_symbols::SymbolHandle,
    contract: &ArrivalRequirementContract,
) -> Result<TargetOwnedPhysicalContractSource, Diagnostic> {
    let expected_package = slot.physical_contract_package.ok_or_else(|| {
        Diagnostic::error("target physical entry requirement has no owning package identity")
    })?;
    let source_span = typed
        .symbols
        .symbol_source_span(contract.signature)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "target physical entry requirement `{}` has no authored source provenance",
                contract.requirement_identity
            ))
        })?;
    let source_file = typed.symbols.source_file(source_span).ok_or_else(|| {
        Diagnostic::error("target physical entry requirement lost its source-file provenance")
    })?;
    let schema_source_span = typed.symbols.symbol_source_span(schema).ok_or_else(|| {
        Diagnostic::error("target physical entry schema has no authored source provenance")
    })?;
    let schema_source_file = typed
        .symbols
        .source_file(schema_source_span)
        .ok_or_else(|| {
            Diagnostic::error("target physical entry schema lost its source-file provenance")
        })?;
    let package_relative_source = source_file
        .path
        .strip_prefix(&source_file.package_root)
        .ok();
    if source_file.origin != psi_source::SourceOrigin::Toolchain
        || schema_source_file.source_id != source_file.source_id
        || package_relative_source
            != Some(std::path::Path::new(
                expected_package.package_relative_source(),
            ))
    {
        return Err(Diagnostic::error(format!(
            "target physical entry requirement and schema `{}` must come from exact toolchain package `{}`, not `{}`",
            contract.requirement_identity,
            expected_package.manifest_identity(),
            source_file.path.display()
        )));
    }
    let package_fingerprint = physical_contract_package_fingerprint(
        expected_package.manifest_identity().as_bytes(),
        source_file.source.as_bytes(),
    );
    Ok(TargetOwnedPhysicalContractSource {
        package: expected_package,
        package_fingerprint,
    })
}

fn physical_contract_package_fingerprint(identity: &[u8], source: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for bytes in [
        b"omega.uefi-physical-package.v1".as_slice(),
        identity,
        source,
    ] {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

struct ArrivalRequirementParameterType {
    identity: psi_typed_trees::type_identity::NormalizedTypeIdentity,
    display: String,
    is_const: bool,
    is_mutable: bool,
}

struct ArrivalRequirementContract {
    signature: psi_symbols::SymbolHandle,
    requirement_identity: String,
    parameters: Vec<ArrivalRequirementParameterType>,
    result_type_identity: Option<psi_typed_trees::type_identity::NormalizedTypeIdentity>,
}

/// Resolve the target declaration back to its core-owned typed requirement.
/// The result is deliberately taken from Psi's normalized identities rather
/// than reconstructed from display strings in the Omega orchestrator.
fn arrival_requirement_contract(
    typed: &TypedTrees,
    requirement: &str,
) -> Result<ArrivalRequirementContract, Diagnostic> {
    let Some((owner, method)) = requirement.split_once("::") else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` is not an exact `Trait::machine` identity"
        )));
    };
    let definitions = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && definition.name.as_str() == owner)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} boundary trait declarations instead of exactly one",
            definitions.len()
        )));
    };
    let signatures = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == method)
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} machine declarations instead of exactly one",
            signatures.len()
        )));
    };
    Ok(ArrivalRequirementContract {
        signature: signature.symbol,
        requirement_identity: typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity(),
        parameters: typed
            .state_signature_parameters(signature)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| ArrivalRequirementParameterType {
                identity: typed.normalized_type_identity(parameter.type_reference),
                display: typed.display_type_reference_with_constraints(parameter.type_reference),
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
            .collect(),
        result_type_identity: signature
            .return_type
            .is_valid()
            .then(|| typed.normalized_type_identity(signature.return_type)),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelectionIdentity {
    pub symbol: SymbolHandle,
    pub package: Option<psi_core::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub boundary_trait: ProviderSelectionIdentity,
    pub provider_type: ProviderSelectionIdentity,
}

#[cfg(test)]
impl ProviderSelection {
    pub(crate) fn exact_for_test(boundary_trait: &str, provider_type: &str) -> Self {
        Self {
            boundary_trait: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: boundary_trait.to_owned(),
                authored_path: boundary_trait.to_owned(),
            },
            provider_type: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: provider_type.to_owned(),
                authored_path: provider_type.to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCompatibilityDemand {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub require_readable: bool,
    pub require_writable: bool,
    pub require_unknown_preservation: bool,
    pub require_canonical: bool,
    pub require_complete_migration: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
            grants: Vec::new(),
            provider_selections: Vec::new(),
            wire_compatibility_demands: Vec::new(),
            root_bindings: Vec::new(),
        }
    }
}

/// Whether a machine is the program's build machine: named `build` (the
/// FREE pure-config shape) or `<Component>::build` (the dependency-
/// injection shape, owner answer #2) AND declared at a build.omg root --
/// the FILE is the identity (owner answer #3: build.omg is the home;
/// `MazeBuilder::build` in ordinary source is just a machine). The caller
/// threads the build-file machine names from the syntax stage, where
/// per-file item attribution exists; typed machines carry no source file.
/// A wrong-arity build machine still refuses at evaluation with the arity
/// error (pinned by fail/build/build_machine_wrong_arity).
pub(crate) fn is_build_machine(
    machine: &psi_typed_trees::machine::Machine,
    build_file_machines: &[String],
) -> bool {
    let name = machine.name.as_str();
    if name != BUILD_MACHINE && !name.ends_with("::build") {
        return false;
    }
    build_file_machines.iter().any(|declared| declared == name)
}

fn is_exact_toolchain_build_service(
    typed: &TypedTrees,
    service: psi_language_semantics::ServiceReachId,
    expected_name: &str,
    expected_source: &str,
) -> bool {
    let Some(definition) = typed.service_reaches.definition(service) else {
        return false;
    };
    definition.name == expected_name
        && typed
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| typed.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == psi_source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new(expected_source))
            })
}

/// Evaluate the program's `build` machine (if any) and extract the config.
/// No `build` machine -> the default. Every failure names the machine.
pub(crate) fn compute_build_config(
    typed: &TypedTrees,
    build_file_machines: &[String],
    filesystem_scope: &BuildMachineFilesystemScope,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let prepared = PreparedBuildMachineProgram::prepare(typed)?;
    let typed = prepared.typed();

    let mut build_machines = typed
        .machines()
        .iter()
        .filter(|machine| is_build_machine(machine, build_file_machines));
    let Some(machine) = build_machines.next() else {
        return Ok(ComputedBuildConfig {
            config: BuildConfig::default(),
            evaluation_usage: None,
            observation_summary: None,
            selected_build_machine_symbol: None,
        });
    };
    if let Some(second) = build_machines.next() {
        return Err(vec![Diagnostic::error(format!(
            "two build machines exist (`{}` and `{}`); a program declares at most one",
            machine.name.as_str(),
            second.name.as_str(),
        ))]);
    }
    let machine_name = machine.name.as_str();

    // The build gate admits exactly the pinned standard staging slots from
    // build_and_package_model.md: FilesystemHost and Console. These are
    // canonical boundary-service identities, never lowercase compatibility
    // categories. Custom boundary wrappers are distinct services and refuse
    // unless the build contract is deliberately extended.
    let effect_plan = psi_effects::infer_operational_may(typed);
    let service_plan = psi_effects::infer_service_reaches(typed, &effect_plan);
    let transitive = service_plan
        .for_machine(machine.symbol)
        .map(|entry| service_plan.services(entry.inferred_transitive))
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
    const ALLOWED_BUILD_SERVICES: &[(&str, &str)] = &[
        ("FilesystemHost", "filesystem_host.omg"),
        ("Console", "console.omg"),
    ];
    let disallowed: Vec<&str> = transitive
        .iter()
        .filter(|service| {
            let Some(definition) = typed.service_reaches.definition(**service) else {
                return true;
            };
            !ALLOWED_BUILD_SERVICES.iter().any(|(name, source)| {
                definition.name == *name
                    && is_exact_toolchain_build_service(typed, **service, name, source)
            })
        })
        .map(|service| {
            typed
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect();
    if !disallowed.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches boundary service{} `{}` -- build.omg may reach only \
             the pinned staging service{} `{}`",
            if disallowed.len() == 1 { "" } else { "s" },
            disallowed.join(", "),
            if ALLOWED_BUILD_SERVICES.len() == 1 {
                ""
            } else {
                "s"
            },
            ALLOWED_BUILD_SERVICES
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join("`, `"),
        ))]);
    }
    // Allowed services must still be authored on the build machine's stable
    // public ceiling; inferred reach cannot silently expand staging authority.
    let declared = typed.service_reach_rows.services(machine.service_reach_row);
    let undeclared: Vec<&str> = transitive
        .iter()
        .filter(|service| !declared.contains(service))
        .map(|service| {
            typed
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect();
    if !undeclared.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches boundary service{} `{}` without declaring {} in its \
             service ceiling; add `reaches {}` to the build machine's signature",
            if undeclared.len() == 1 { "" } else { "s" },
            undeclared.join(", "),
            if undeclared.len() == 1 { "it" } else { "them" },
            undeclared.join(", "),
        ))]);
    }

    let zero_build = BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: vec![
            (
                "subsystem".to_owned(),
                BuildTimeValue::Case {
                    variant: "Console".to_owned(),
                    payload: Vec::new(),
                },
            ),
            ("freestanding".to_owned(), BuildTimeValue::Bool(false)),
        ],
    };

    let filesystem_reachable = transitive.iter().any(|service| {
        is_exact_toolchain_build_service(typed, *service, "FilesystemHost", "filesystem_host.omg")
    });

    // Omega owns the grant decision. Psi owns the target-neutral interpreter
    // entry selected by that explicit mode. Console needs the granted entry so
    // its output can be served, but it must not incidentally install real
    // filesystem authority.
    let execution_mode = if transitive.is_empty() {
        BuildMachineExecutionMode::Pure
    } else {
        let filesystem = if filesystem_reachable {
            filesystem_scope.ensure_write_roots()?;
            BuildMachineFilesystemAccess::RealScoped(filesystem_scope.grants())
        } else {
            BuildMachineFilesystemAccess::Virtual
        };
        BuildMachineExecutionMode::Granted { filesystem }
    };
    let measured = psi_build_time_evaluation::evaluate_build_machine_arguments_measured(
        &prepared,
        machine_name,
        vec![zero_build],
        execution_mode,
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}"
        ))]
    })?;
    let usage = measured.usage();
    let observation_ceiling = if filesystem_reachable {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    let realized_observation = if measured.observations().filesystem_host_observed() {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    if realized_observation > observation_ceiling {
        return Err(vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` observed filesystem host state outside its static observation ceiling"
        ))]);
    }
    let mut arguments = measured.into_value();
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let mut config = extract_build_config(&augmented).map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` produced an invalid Build: {reason}"
        ))]
    })?;
    config.grants = harvest_root_grants(typed, machine);
    config.provider_selections = harvest_provider_selections(typed, machine)?;
    config.wire_compatibility_demands = harvest_wire_compatibility_demands(typed, machine)?;
    config.root_bindings = harvest_root_bindings(typed, machine)?;
    Ok(ComputedBuildConfig {
        config,
        evaluation_usage: Some(BuildEvaluationUsage {
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            fuel_units: usage.fuel_units(),
            result_cells: usage.result_cells(),
        }),
        observation_summary: Some(BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            ceiling: observation_ceiling,
            realized: realized_observation,
        }),
        selected_build_machine_symbol: Some(machine.symbol),
    })
}

/// Collect `builder.roots.bind(Target::Slot, Machine::entry);` declarations
/// from the one authoritative build machine. Slot membership and schema
/// checking belong to the selected target profile; this stage establishes the
/// closed, duplicate-free binding map and preserves the exact machine name.
fn harvest_root_bindings(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let mut bindings: Vec<RootBinding> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str| {
        let Some(encoded) = target.strip_prefix("bind_root#") else {
            return;
        };
        let Some((slot, implementation)) = encoded.split_once('#') else {
            diagnostics.push(Diagnostic::error(format!(
                "malformed root-slot binding declaration `{target}`"
            )));
            return;
        };
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`",
                existing.implementation
            )));
            return;
        }
        bindings.push(RootBinding {
            slot: slot.to_owned(),
            implementation: implementation.to_owned(),
        });
    };

    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                psi_typed_trees::statement::StatementNode::Expression(expression) => {
                    if let psi_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(call.target.as_str());
                    }
                }
                psi_typed_trees::statement::StatementNode::Call(call) => {
                    record(call.target.as_str());
                }
                _ => {}
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(bindings)
    } else {
        Err(diagnostics)
    }
}

/// Chapter 21: collect the edge-specific wire facts requested by the one
/// authoritative build machine. The parser has already validated the closed
/// fact vocabulary; this pass validates the marker encoding and duplicate
/// declarations before compatibility evaluation consumes it.
fn harvest_wire_compatibility_demands(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<WireCompatibilityDemand>, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str| {
        let Some(encoded) = target.strip_prefix("wire_compatibility#") else {
            return;
        };
        let parts = encoded.split('#').collect::<Vec<_>>();
        if parts.len() < 5 {
            diagnostics.push(Diagnostic::error(format!(
                "malformed wire compatibility declaration `{target}`"
            )));
            return;
        }
        let mut demand = WireCompatibilityDemand {
            edge: parts[0].to_owned(),
            lineage: parts[1].to_owned(),
            local_schema: parts[2].to_owned(),
            peer_schema: parts[3].to_owned(),
            require_readable: false,
            require_writable: false,
            require_unknown_preservation: false,
            require_canonical: false,
            require_complete_migration: false,
        };
        for fact in &parts[4..] {
            match *fact {
                "Readable" => demand.require_readable = true,
                "Writable" => demand.require_writable = true,
                "PreserveUnknown" => demand.require_unknown_preservation = true,
                "Canonical" => demand.require_canonical = true,
                "CompleteMigration" => demand.require_complete_migration = true,
                other => diagnostics.push(Diagnostic::error(format!(
                    "malformed wire compatibility declaration `{target}`: unknown fact `{other}`"
                ))),
            }
        }
        if demands.iter().any(|existing: &WireCompatibilityDemand| {
            existing.edge == demand.edge
                && existing.lineage == demand.lineage
                && existing.local_schema == demand.local_schema
                && existing.peer_schema == demand.peer_schema
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "wire compatibility demand for edge `{}`, lineage `{}`, local schema `{}`, \
                 and peer schema `{}` is declared twice",
                demand.edge, demand.lineage, demand.local_schema, demand.peer_schema
            )));
            return;
        }
        demands.push(demand);
    };

    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                psi_typed_trees::statement::StatementNode::Expression(expression) => {
                    if let psi_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(call.target.as_str());
                    }
                }
                psi_typed_trees::statement::StatementNode::Call(call) => {
                    record(call.target.as_str());
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(demands)
    } else {
        Err(diagnostics)
    }
}

/// PRV4c: collect `b.select_provider<BoundaryTrait, ProviderType>();` from
/// the one authoritative build machine. Merely spelling either type elsewhere
/// grants nothing; selection authority comes from this file-scoped root.
fn harvest_provider_selections(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut selections: Vec<ProviderSelection> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record =
        |target: &str, arguments: &[psi_typed_trees::expression::StaticMachineArgument]| {
            if target != "select_provider" {
                return;
            }
            let [boundary_argument, provider_argument] = arguments else {
                diagnostics.push(Diagnostic::error(
                    "provider selection must retain exactly two resolved type paths",
                ));
                return;
            };
            let project_identity =
                |argument: &psi_typed_trees::expression::StaticMachineArgument| {
                    let authored_path = argument
                        .path
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    ProviderSelectionIdentity {
                        symbol: argument.symbol,
                        package: typed.symbols.symbol_package_identity(argument.symbol),
                        canonical_path: typed.symbols.display_path(argument.symbol, "::"),
                        authored_path,
                    }
                };
            let boundary_trait = project_identity(boundary_argument);
            let provider_type = project_identity(provider_argument);
            if !boundary_trait.symbol.is_valid()
                || typed.symbols.get(boundary_trait.symbol).kind != SymbolKind::Trait
                || !typed.traits().iter().any(|definition| {
                    definition.symbol == boundary_trait.symbol && definition.is_boundary
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "provider selection boundary `{}` does not resolve to an exact boundary trait",
                    boundary_trait.authored_path
                )));
                return;
            }
            if !provider_type.symbol.is_valid()
                || typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
            {
                diagnostics.push(Diagnostic::error(format!(
                    "provider selection type `{}` does not resolve to an exact data declaration",
                    provider_type.authored_path
                )));
                return;
            }
            if let Some(existing) = selections
                .iter()
                .find(|selection| selection.boundary_trait.symbol == boundary_trait.symbol)
            {
                if existing.provider_type.symbol != provider_type.symbol {
                    diagnostics.push(Diagnostic::error(format!(
                        "build selects two provider types for slot `{}`: `{}` and `{}`",
                        boundary_trait.canonical_path,
                        existing.provider_type.canonical_path,
                        provider_type.canonical_path,
                    )));
                }
                return;
            }
            selections.push(ProviderSelection {
                boundary_trait,
                provider_type,
            });
        };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                psi_typed_trees::statement::StatementNode::Expression(expression) => {
                    if let psi_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(call.target.as_str(), &call.machine_arguments);
                    }
                }
                psi_typed_trees::statement::StatementNode::Call(call) => {
                    record(call.target.as_str(), &call.machine_arguments);
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

/// PRV4c: collect the defaults declared by the selected target package(s).
/// `target_machines` records the authoritative machine names before erasing
/// their target markers. The declarations use the same type-per-slot marker as
/// build overrides, but retain distinct provenance so selection can apply the
/// precedence `build override > target default > unique declaration default`.
pub(crate) fn compute_target_provider_defaults(
    typed: &TypedTrees,
    target_default_machine_names: &[String],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut defaults = Vec::new();
    let mut diagnostics = Vec::new();
    for machine_name in target_default_machine_names {
        let Some(machine) = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected target provider-default machine `{machine_name}` did not survive lowering"
            )));
            continue;
        };
        match harvest_provider_selections(typed, machine) {
            Ok(mut machine_defaults) => defaults.append(&mut machine_defaults),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(defaults)
    } else {
        Err(diagnostics)
    }
}

/// The static grant harvest: every `accept_boundary#<path>` marker call in
/// the build machine's states (the postfix carve's desugar of
/// `b.accept_boundary<path>();`). Order-preserving, deduplicated.
fn harvest_root_grants(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<String> {
    let mut grants: Vec<String> = Vec::new();
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            let handles: Vec<psi_typed_trees::expression::ExpressionHandle> = match statement {
                psi_typed_trees::statement::StatementNode::Expression(expression) => {
                    vec![*expression]
                }
                psi_typed_trees::statement::StatementNode::Call(call) => {
                    // A statement-level call keeps the marker in its target.
                    if let Some(path) = call.target.as_str().strip_prefix("accept_boundary#")
                        && !grants.iter().any(|existing| existing == path)
                    {
                        grants.push(path.to_owned());
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                if let psi_typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(handle)
                    && let Some(path) = call.target.as_str().strip_prefix("accept_boundary#")
                    && !grants.iter().any(|existing| existing == path)
                {
                    grants.push(path.to_owned());
                }
            }
        }
    }
    grants
}

fn extract_build_config(build: &BuildTimeValue) -> Result<BuildConfig, String> {
    let BuildTimeValue::Struct { fields, .. } = build else {
        return Err(format!("expected a Build struct, got {build:?}"));
    };
    let field = |name: &str| -> Result<&BuildTimeValue, String> {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
            .ok_or_else(|| format!("the Build carries no `{name}` field"))
    };

    let subsystem = match field("subsystem")? {
        BuildTimeValue::Case { variant, payload } => {
            match variant.rsplit("::").next().unwrap_or(variant) {
                "Console" => 3u16,
                "Gui" => 2,
                "EfiApplication" => 10,
                "Unspecified" => match payload.iter().find(|(name, _)| name == "value") {
                    Some((_, BuildTimeValue::Int(value))) => {
                        u16::try_from(*value).map_err(|_| {
                            format!("Unspecified subsystem value {value} exceeds a u16")
                        })?
                    }
                    other => {
                        return Err(format!(
                            "Unspecified subsystem carries no integer value: {other:?}"
                        ));
                    }
                },
                other => return Err(format!("unknown Subsystem case `{other}`")),
            }
        }
        other => {
            return Err(format!(
                "Build.subsystem is not a Subsystem case: {other:?}"
            ));
        }
    };

    let freestanding = match field("freestanding")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    };

    Ok(BuildConfig {
        subsystem,
        freestanding,
        grants: Vec::new(),
        provider_selections: Vec::new(),
        wire_compatibility_demands: Vec::new(),
        root_bindings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{BuildConfig, RootBinding, selected_program_entry_machine};

    fn config_with_root_bindings(bindings: &[(&str, &str)]) -> BuildConfig {
        BuildConfig {
            root_bindings: bindings
                .iter()
                .map(|(slot, implementation)| RootBinding {
                    slot: (*slot).to_owned(),
                    implementation: (*implementation).to_owned(),
                })
                .collect(),
            ..BuildConfig::default()
        }
    }

    #[test]
    fn selected_target_ignores_valid_foreign_program_entry_slot_after_its_own() {
        let config = config_with_root_bindings(&[
            ("windows_x86_64::ProgramEntry", "Application::start"),
            ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
        ]);

        let selected = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect("known foreign target roots remain available to their own profiles")
            .expect("selected target has one exact root");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, omega_target::TargetProfile::WindowsX64);
    }

    #[test]
    fn selected_target_ignores_valid_foreign_program_entry_slot_before_its_own() {
        let config = config_with_root_bindings(&[
            ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
            ("windows_x86_64::ProgramEntry", "Application::start"),
        ]);

        let selected = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect("binding order cannot change target-scoped selection")
            .expect("selected target has one exact root");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, omega_target::TargetProfile::WindowsX64);
    }

    #[test]
    fn selected_entry_retains_the_target_owned_slot_schema() {
        let config =
            config_with_root_bindings(&[("uefi_x86_64::ProgramEntry", "Application::start")]);

        let selected = selected_program_entry_machine(&config, Some("uefi_x64"))
            .expect("typed root slot selection")
            .expect("one selected entry");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, omega_target::TargetProfile::UefiX64);
        assert_eq!(
            selected.slot.visible_parameters,
            omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        );
    }

    #[test]
    fn root_slot_owner_rejects_deployment_target_names() {
        let config =
            config_with_root_bindings(&[("windows_x64::ProgramEntry", "Application::start")]);

        let diagnostics = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect_err("a noncanonical target owner must reject");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "root slot `windows_x64::ProgramEntry` belongs to unknown target profile `windows_x64`"
        ));
    }

    #[test]
    fn root_selection_rejects_names_absent_from_the_target_catalog() {
        let config =
            config_with_root_bindings(&[("windows_x86_64::UndeclaredEntry", "Application::start")]);

        let diagnostics = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect_err("an undeclared target root cannot enter ProgramEntry lowering");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "target profile `windows_x64` declares no required root slot `windows_x86_64::UndeclaredEntry`"
        ));
    }

    #[test]
    fn selected_target_requires_every_member_of_its_catalog() {
        let config =
            config_with_root_bindings(&[("linux_x86_64::ProgramEntry", "Diagnostics::start")]);

        let diagnostics = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect_err("a foreign target row cannot satisfy the selected catalog");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "selected target `windows_x64` has no bound required root slot `windows_x86_64::ProgramEntry`"
        ));
    }

    #[test]
    fn selected_target_rejects_duplicate_catalog_members() {
        let config = config_with_root_bindings(&[
            ("windows_x86_64::ProgramEntry", "Application::start"),
            ("windows_x86_64::ProgramEntry", "Diagnostics::start"),
        ]);

        let diagnostics = selected_program_entry_machine(&config, Some("windows_x64"))
            .expect_err("one required catalog member cannot be bound twice");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "selected target `windows_x64` has more than one bound required root slot `windows_x86_64::ProgramEntry`"
        ));
    }
}
