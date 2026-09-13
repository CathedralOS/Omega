//! Program-entry selection and validation of source and physical calling contracts.

use crate::BuildConfig;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(super) mod root_bindings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    /// Exact machine selected under the binding occurrence's lexical package.
    pub machine_symbol: symbols::SymbolHandle,
    pub slot: target::ProgramEntrySlotDeclaration,
}

/// Resolve the selected target's `ProgramEntry` binding. This is the first
/// implemented target-root slot; other root-slot kinds reject rather than
/// being accepted and then ignored. A build file may describe a target matrix:
/// well-formed slots owned by other known profiles are validated and left for
/// those profiles, while this selection consumes only the chosen profile's
/// exact row. With no root declarations at all the caller may still enter the
/// explicit migration fallback for the remaining corpus.
pub fn selected_program_entry_machine<'config>(
    config: &'config BuildConfig,
    selected_profile: Option<target::TargetProfile>,
) -> Result<Option<SelectedProgramEntry<'config>>, Vec<Diagnostic>> {
    let Some(selected_profile) = selected_profile else {
        return Ok(None);
    };
    if config.root_bindings.is_empty() {
        return Ok(None);
    }
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
        let profile = match target::TargetProfile::from_root_slot_owner(profile) {
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
            machine_symbol: binding.implementation_symbol,
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
pub fn validate_selected_program_entry_shape(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
) -> Result<program_entry_plan::SelectedProgramEntrySourceSignature, Vec<Diagnostic>> {
    let machine_name = selected.machine_name;
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == selected.machine_symbol)
    else {
        return Err(vec![Diagnostic::error(format!(
            "build root slot selected entry `{machine_name}` is not a declaration in the admitted program"
        ))]);
    };
    if typed
        .machines()
        .iter()
        .any(|other| other.symbol != machine.symbol && other.name.as_str() == machine.name.as_str())
    {
        return Err(vec![Diagnostic::error(format!(
            "root slot binds `{machine_name}` exactly, but another package declares a same-named machine; name-keyed Terminal production cannot rejoin the exact selected identity yet"
        ))]);
    }
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
        && typed_trees_to_checked_trees::data_requires_establishment(typed, definition)
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
        target::ProgramEntryVisibleParameters::None if !visible.is_empty() => {
            diagnostics.push(Diagnostic::error(format!(
                "hosted `ProgramEntry` exposes no arrival parameters, but `{machine_name}` declares `{}`",
                visible
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>()
                    .join("`, `")
            )));
        }
        target::ProgramEntryVisibleParameters::ImageAndInitialStorage if visible.len() != 2 => {
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
        == target::ProgramEntryVisibleParameters::ImageAndInitialStorage
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
        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
        |receiver| program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
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
                0 => program_entry_plan::ProgramStorageEntryRootRole::Image,
                1 => program_entry_plan::ProgramStorageEntryRootRole::InitialStorage,
                _ => unreachable!("selected source shape validation fixed visible arity"),
            };
            let extent_value_layout =
                provider_planning::calling_policy_plans::selected_program_storage_source_extent_value_layout(
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
            Ok(program_entry_plan::SelectedProgramEntrySourceSignature::visible_parameter(
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
    program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
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

/// Retain both authored applications: their source/signature commitments are
/// distinct from the validated ABI plans' own structural commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCallingPlans {
    pub semantic_calling_application:
        provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
    pub physical_calling_application:
        provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
    pub storage_entry: program_entry_plan::SelectedProgramStorageEntryPlan,
}

/// Complete build-owned settlement for one target-selected `ProgramEntry`.
///
/// The source signature and optional two-surface calling plans are validated
/// while typed declarations are still available, then travel together instead
/// of becoming independent driver couriers. A test-harness entry-name override
/// is deliberately absent: it cannot acquire source, calling-plan, or storage
/// authority through this carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerProgramEntry {
    source_signature: program_entry_plan::SelectedProgramEntrySourceSignature,
    calling_plans: Option<SelectedProgramEntryCallingPlans>,
    fused_service_establishments: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
}

impl SelectedCompilerProgramEntry {
    fn new(
        source_signature: program_entry_plan::SelectedProgramEntrySourceSignature,
        calling_plans: Option<SelectedProgramEntryCallingPlans>,
    ) -> Self {
        Self {
            source_signature,
            calling_plans,
            fused_service_establishments: Vec::new(),
        }
    }

    pub fn machine_name(&self) -> &str {
        self.source_signature.machine_name()
    }

    pub const fn source_signature(
        &self,
    ) -> &program_entry_plan::SelectedProgramEntrySourceSignature {
        &self.source_signature
    }

    pub const fn calling_plans(&self) -> Option<&SelectedProgramEntryCallingPlans> {
        self.calling_plans.as_ref()
    }

    pub fn fused_service_establishments(
        &self,
    ) -> &[program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        &self.fused_service_establishments
    }

    pub fn bind_fused_service_establishments(
        &mut self,
        mut establishments: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    ) -> Result<(), &'static str> {
        if establishments.is_empty() {
            self.fused_service_establishments.clear();
            return Ok(());
        }
        let source_identity = self.source_signature.identity();
        let target_slot = self.source_signature.target_slot();
        let receiver_identity = self
            .source_signature
            .receiver()
            .normalized_type_identity()
            .ok_or("a free ProgramEntry cannot establish receiver fields")?;
        establishments.sort_by(|left, right| left.field_identity().cmp(right.field_identity()));
        if establishments
            .windows(2)
            .any(|pair| pair[0].field_identity() == pair[1].field_identity())
            || establishments.iter().any(|establishment| {
                establishment.source_signature_identity() != source_identity
                    || establishment.target_slot() != target_slot
                    || establishment.receiver_type_identity() != receiver_identity
            })
        {
            return Err("Fused root establishments drifted from their selected ProgramEntry");
        }
        self.fused_service_establishments = establishments;
        Ok(())
    }

    pub fn into_parts(
        self,
    ) -> (
        program_entry_plan::SelectedProgramEntrySourceSignature,
        Option<SelectedProgramEntryCallingPlans>,
        Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    ) {
        (
            self.source_signature,
            self.calling_plans,
            self.fused_service_establishments,
        )
    }
}

/// Resolve and validate the complete build-owned `ProgramEntry` input.
/// Diagnostic order is intentional: exact target-slot selection precedes
/// source-shape validation, which precedes optional physical/semantic calling-
/// plan validation.
pub fn select_compiler_program_entry(
    typed: &TypedTrees,
    config: &BuildConfig,
    selected_profile: Option<target::TargetProfile>,
    realizations: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    accepted_uefi_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<Option<SelectedCompilerProgramEntry>, Vec<Diagnostic>> {
    let Some(selected) = selected_program_entry_machine(config, selected_profile)? else {
        return Ok(None);
    };
    let source_signature = validate_selected_program_entry_shape(typed, selected)?;
    let calling_plans = validate_selected_program_entry_calling_plan(
        typed,
        selected,
        realizations,
        accepted_uefi_binding,
    )?;
    Ok(Some(SelectedCompilerProgramEntry::new(
        source_signature,
        calling_plans,
    )))
}

pub fn validate_selected_program_entry_calling_plan(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
    realizations: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    accepted_uefi_binding: Option<&package_compilation::AcceptedSemanticBinding>,
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
    for (role, realization) in [
        ("semantic", *semantic_realization),
        ("physical", *physical_realization),
    ] {
        let (validated, application_report_fingerprint, application_commitment) = realization
            .replayed_validated_application()
            .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` retains an invalid {role} calling plan: {error}"
            ))]
        })?;
        if realization.exact_boundary_entry_plan() != validated.plan()
            || realization.report_fingerprint != application_report_fingerprint
            || realization.commitment != application_commitment
        {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` does not retain exact {role} calling-plan identity and custody"
            ))]);
        }
    }
    let (validated_physical_plan, _, _) = physical_realization
        .replayed_validated_application()
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` lost its validated physical calling plan: {error}"
            ))]
        })?;
    let expected_physical = match physical_convention {
        target::ProgramEntryCallingConvention::MicrosoftX64 => {
            calling_conventions::CallingPolicy::MicrosoftX64
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
        target::ProgramEntryCallingConvention::MicrosoftX64 => {
            calling_conventions::CallingPolicy::MicrosoftX64
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
    let service_schema =
        provider_planning::service_schema::from_typed(typed, schema).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "target entry schema `{schema_name}` is not a boundary service schema"
            ))]
        })?;
    let physical_source = target_owned_physical_contract_source(
        typed,
        selected.slot,
        schema.symbol,
        &service_schema,
        &physical,
        accepted_uefi_binding,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let storage_entry = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
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
    let physical_contract = program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        selected.slot,
        physical.requirement_identity,
        physical_source.package,
        physical_source.package_source_digest,
        physical_source.non_authoritative_package_source_report_fingerprint,
        physical
            .parameters
            .into_iter()
            .map(|parameter| parameter.identity.into_string())
            .collect(),
        result_type_identity.into_string(),
        validated_physical_plan.contract_report_fingerprint(),
        physical_realization.boundary_entry_plan.clone(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    let storage_entry = storage_entry
        .with_physical_contract(physical_contract.clone())
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    Ok(Some(SelectedProgramEntryCallingPlans {
        semantic_calling_application: (*semantic_realization).clone(),
        physical_calling_application: (*physical_realization).clone(),
        storage_entry,
    }))
}

struct TargetOwnedPhysicalContractSource {
    package: target::ProgramEntryPhysicalContractPackage,
    package_source_digest: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_package_source_report_fingerprint: u64,
}

fn target_owned_physical_contract_source(
    typed: &TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    schema: symbols::SymbolHandle,
    service_schema: &effects::provider_plan::ServiceSchema,
    contract: &ArrivalRequirementContract,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
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
    let package_source_digest =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            expected_package,
            source_file.source.as_bytes(),
        );
    if schema_source_file.source_id != source_file.source_id {
        return Err(Diagnostic::error(format!(
            "target physical entry requirement `{}` and its selected schema must come from one exact source unit",
            contract.requirement_identity,
        )));
    }
    match accepted_binding {
        Some(binding) => {
            let exact_package_binding = binding.role()
                == package_compilation::AcceptedSemanticBindingRole::UefiX64ProgramEntry
                && binding.selected_provider_plan_digest().is_none()
                && source_file.package_identity == Some(binding.package())
                && schema_source_file.package_identity == Some(binding.package())
                && typed.symbols.symbol_package_identity(schema) == Some(binding.package())
                && typed.symbols.display_path(schema, "::") == binding.declaration_path()
                && service_schema.trait_package_identity == Some(binding.package())
                && package_compilation::accepted_service_schema_digest(
                    binding.role(),
                    service_schema,
                ) == binding.normalized_schema_digest();
            if !exact_package_binding {
                return Err(Diagnostic::error(format!(
                    "target physical entry schema `{}` does not match the exact accepted UEFI package binding",
                    typed.symbols.display_path(schema, "::"),
                )));
            }
        }
        None => {
            let exact_bundled_source = source_file.package_identity.is_none()
                && schema_source_file.package_identity.is_none()
                && package_relative_source
                    == Some(std::path::Path::new(
                        expected_package.package_relative_source(),
                    ))
                && package_source_digest
                    == program_entry_plan::exact_uefi_x64_physical_contract_package_source_digest();
            if !exact_bundled_source {
                return Err(Diagnostic::error(format!(
                    "target physical entry requirement and schema `{}` require either the exact bundled UEFI contract or one accepted package-owned UEFI binding, not `{}`",
                    contract.requirement_identity,
                    source_file.path.display(),
                )));
            }
        }
    }
    let package_source_report_fingerprint = physical_contract_package_source_report_fingerprint(
        expected_package.manifest_identity().as_bytes(),
        source_file.source.as_bytes(),
    );
    Ok(TargetOwnedPhysicalContractSource {
        package: expected_package,
        package_source_digest,
        non_authoritative_package_source_report_fingerprint: package_source_report_fingerprint,
    })
}

fn physical_contract_package_source_report_fingerprint(identity: &[u8], source: &[u8]) -> u64 {
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
    identity: typed_trees::type_identity::NormalizedTypeIdentity,
    display: String,
    is_const: bool,
    is_mutable: bool,
}

struct ArrivalRequirementContract {
    signature: symbols::SymbolHandle,
    requirement_identity: String,
    parameters: Vec<ArrivalRequirementParameterType>,
    result_type_identity: Option<typed_trees::type_identity::NormalizedTypeIdentity>,
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

#[cfg(test)]
mod tests {
    use super::{
        SelectedCompilerProgramEntry, select_compiler_program_entry, selected_program_entry_machine,
    };
    use crate::{BuildConfig, RootBinding};

    fn config_with_root_bindings(bindings: &[(&str, &str)]) -> BuildConfig {
        BuildConfig {
            root_bindings: bindings
                .iter()
                .map(|(slot, implementation)| RootBinding {
                    slot: (*slot).to_owned(),
                    implementation: (*implementation).to_owned(),
                    implementation_symbol: symbols::SymbolHandle::from_arena_index(1),
                })
                .collect(),
            ..BuildConfig::default()
        }
    }

    fn source_only_program_entry_settlement() -> SelectedCompilerProgramEntry {
        let source_signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target::TargetProfile::WindowsX64.program_entry_slot(),
                symbols::SymbolHandle::from_arena_index(1),
                symbols::SymbolHandle::from_arena_index(2),
                "Application::start".into(),
                "entry".into(),
                "Application::start::entry() -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .expect("exact source-only ProgramEntry fixture");
        SelectedCompilerProgramEntry::new(source_signature, None)
    }

    fn provisioned_program_entry_settlement() -> SelectedCompilerProgramEntry {
        let source_signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target::TargetProfile::WindowsX64.program_entry_slot(),
                symbols::SymbolHandle::from_arena_index(1),
                symbols::SymbolHandle::from_arena_index(2),
                "Application::start".into(),
                "entry".into(),
                "Application::start::entry(&mut self) -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity: "ref-mut(named(name(Application)))".into(),
                },
                Vec::new(),
            )
            .expect("exact provisioned ProgramEntry fixture");
        SelectedCompilerProgramEntry::new(source_signature, None)
    }

    fn fused_root_row(
        selected: &SelectedCompilerProgramEntry,
        field: &str,
    ) -> program_entry_plan::ProgramEntryFusedServiceEstablishment {
        program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
            selected.source_signature().identity(),
            selected.source_signature().target_slot(),
            "ref-mut(named(name(Application)))".into(),
            "named(name(Application))".into(),
            field.into(),
            "qualified(named(name(Service<Console>)), declared-domain(name(Bound)))".into(),
            "named(name(Service<Console>))".into(),
            "Bound".into(),
            "Console".into(),
            effects::provider_plan::ServiceSchemaDigest::from_digest([1; 32]),
            effects::provider_plan::ProviderPlanDigest::from_digest([2; 32]),
        )
        .expect("exact Fused root fixture")
    }

    #[test]
    fn compiler_program_entry_absence_needs_no_typed_or_calling_plan_facts() {
        let selected = select_compiler_program_entry(
            &typed_trees::TypedTrees::default(),
            &BuildConfig::default(),
            None,
            &[],
            None,
        )
        .expect("an absent ProgramEntry is a complete settlement");

        assert!(selected.is_none());
    }

    #[test]
    fn compiler_program_entry_consuming_split_preserves_source_only_custody() {
        let selected = source_only_program_entry_settlement();
        let expected_source = selected.source_signature().clone();

        assert_eq!(selected.machine_name(), "Application::start");
        assert!(selected.calling_plans().is_none());
        let (source_signature, calling_plans, establishments) = selected.into_parts();

        assert_eq!(source_signature, expected_source);
        assert!(calling_plans.is_none());
        assert!(establishments.is_empty());
    }

    #[test]
    fn compiler_program_entry_binds_exact_sorted_fused_root_establishments() {
        let mut selected = provisioned_program_entry_settlement();
        let second = fused_root_row(&selected, "#2");
        let first = fused_root_row(&selected, "#1");
        selected
            .bind_fused_service_establishments(vec![second, first.clone()])
            .expect("exact provisioned roots should bind");
        assert_eq!(
            selected
                .fused_service_establishments()
                .iter()
                .map(|row| row.field_identity())
                .collect::<Vec<_>>(),
            ["#1", "#2"],
        );

        assert!(
            selected
                .bind_fused_service_establishments(vec![first.clone(), first])
                .is_err(),
            "duplicate direct receiver fields must reject"
        );
        let mut free = source_only_program_entry_settlement();
        assert!(
            free.bind_fused_service_establishments(vec![fused_root_row(&selected, "#3")])
                .is_err(),
            "a free ProgramEntry cannot acquire receiver establishment"
        );
    }

    #[test]
    fn compiler_program_entry_validates_source_before_calling_plans() {
        let config = config_with_root_bindings(&[(
            "windows_x86_64::ProgramEntry",
            "MissingApplication::start",
        )]);
        let result = select_compiler_program_entry(
            &typed_trees::TypedTrees::default(),
            &config,
            Some(target::TargetProfile::WindowsX64),
            &[],
            None,
        );
        let Err(diagnostics) = result else {
            panic!("missing source entry must reject before calling-plan selection")
        };

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].to_string(),
            "error: build root slot selected entry `MissingApplication::start` is not a declaration in the admitted program"
        );
    }

    #[test]
    fn targetless_check_does_not_select_a_program_entry_from_retained_bindings() {
        let config =
            config_with_root_bindings(&[("windows_x86_64::ProgramEntry", "Application::start")]);

        assert_eq!(
            selected_program_entry_machine(&config, None)
                .expect("targetless checking is entry-agnostic"),
            None
        );
    }

    #[test]
    fn selected_target_ignores_valid_foreign_program_entry_slot_after_its_own() {
        let config = config_with_root_bindings(&[
            ("windows_x86_64::ProgramEntry", "Application::start"),
            ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
        ]);

        let selected =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
                .expect("known foreign target roots remain available to their own profiles")
                .expect("selected target has one exact root");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, target::TargetProfile::WindowsX64);
    }

    #[test]
    fn selected_target_ignores_valid_foreign_program_entry_slot_before_its_own() {
        let config = config_with_root_bindings(&[
            ("linux_x86_64::ProgramEntry", "Diagnostics::start"),
            ("windows_x86_64::ProgramEntry", "Application::start"),
        ]);

        let selected =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
                .expect("binding order cannot change target-scoped selection")
                .expect("selected target has one exact root");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, target::TargetProfile::WindowsX64);
    }

    #[test]
    fn selected_entry_retains_the_target_owned_slot_schema() {
        let config =
            config_with_root_bindings(&[("uefi_x86_64::ProgramEntry", "Application::start")]);

        let selected =
            selected_program_entry_machine(&config, Some(target::TargetProfile::UefiX64))
                .expect("typed root slot selection")
                .expect("one selected entry");

        assert_eq!(selected.machine_name, "Application::start");
        assert_eq!(selected.slot.owner, target::TargetProfile::UefiX64);
        assert_eq!(
            selected.slot.visible_parameters,
            target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        );
    }

    #[test]
    fn root_slot_owner_rejects_legacy_cli_aliases() {
        let config =
            config_with_root_bindings(&[("windows_x64::ProgramEntry", "Application::start")]);

        let diagnostics =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
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

        let diagnostics =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
                .expect_err("an undeclared target root cannot enter ProgramEntry lowering");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "target profile `windows_x86_64` declares no required root slot `windows_x86_64::UndeclaredEntry`"
        ));
    }

    #[test]
    fn selected_target_requires_every_member_of_its_catalog() {
        let config =
            config_with_root_bindings(&[("linux_x86_64::ProgramEntry", "Diagnostics::start")]);

        let diagnostics =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
                .expect_err("a foreign target row cannot satisfy the selected catalog");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "selected target `windows_x86_64` has no bound required root slot `windows_x86_64::ProgramEntry`"
        ));
    }

    #[test]
    fn selected_target_rejects_duplicate_catalog_members() {
        let config = config_with_root_bindings(&[
            ("windows_x86_64::ProgramEntry", "Application::start"),
            ("windows_x86_64::ProgramEntry", "Diagnostics::start"),
        ]);

        let diagnostics =
            selected_program_entry_machine(&config, Some(target::TargetProfile::WindowsX64))
                .expect_err("one required catalog member cannot be bound twice");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].to_string().contains(
            "selected target `windows_x86_64` has more than one bound required root slot `windows_x86_64::ProgramEntry`"
        ));
    }
}
