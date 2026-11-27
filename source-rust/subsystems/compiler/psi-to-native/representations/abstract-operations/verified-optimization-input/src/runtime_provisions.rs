//! Check the host provision roster against an admitted program and retain its runtime facts.

use crate::{AdmittedRuntimeInput, NativeProgramEntrySettlement};
use calling_conventions::BoundaryCallingApplication;
use psi_entry_memory::CheckedPrivateEntryRamRecipe;
use psi_scalar_applications::AdmittedScalarApplications;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarType};
use std::sync::Arc;
use target_provider_admission::TerminalTargetProviderProposal;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind, TerminalPsiIdentity};

/// Canonical scalar formals for one admitted call. Runtime admission keeps
/// the exact calling application and locator together; phase 03 uses them to
/// request an adapter after the admission of the host roster succeeds.
#[derive(Debug, Clone)]
pub struct AdmittedRuntimeForeignCall {
    formal_count: usize,
    scalar_formals: Vec<(usize, ScalarType)>,
    application: BoundaryCallingApplication,
    locator: target::NormalizedForeignLocator,
}

impl AdmittedRuntimeForeignCall {
    fn from_admission(
        declaration: &BoundaryMachineDeclaration,
        application: &BoundaryCallingApplication,
        locator: &target::NormalizedForeignLocator,
    ) -> Result<Self, String> {
        if !declaration.has_valid_parameter_order() {
            return Err(
                "foreign frame scalar transport has no complete semantic formal order".into(),
            );
        }
        let scalar_formals = declaration
            .parameter_positions(BoundaryParameterKind::Scalar)
            .zip(declaration.scalar_parameters.iter().copied())
            .collect();
        Ok(Self {
            formal_count: declaration.parameter_order.len(),
            scalar_formals,
            application: application.clone(),
            locator: locator.clone(),
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        usize,
        Vec<(usize, ScalarType)>,
        BoundaryCallingApplication,
        target::NormalizedForeignLocator,
    ) {
        (
            self.formal_count,
            self.scalar_formals,
            self.application,
            self.locator,
        )
    }
}

fn capture_scalar_applications(
    input: AdmittedRuntimeInput<'_>,
    identity: TerminalPsiIdentity,
    target: target::TargetProfile,
    installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
) -> Result<Option<AdmittedScalarApplications>, &'static str> {
    if input.identity() != identity
        || input.target_context().is_none_or(|selected| {
            selected.terminal_psi() != identity || selected.target() != target.target_name()
        })
    {
        return Err("runtime capture names another native artifact or target");
    }
    let Some(completed) = input.scalar_applications() else {
        return Ok(None);
    };
    if completed.terminal() != identity
        || completed
            .primary()
            .targets()
            .map(|selected| selected.target())
            != Some(target.target_name())
        || installation
            .is_some_and(|installed| !installed.is_subset_of(completed.primary().providers()))
    {
        return Err("runtime scalar capture differs from the emitted provider installation");
    }
    Ok(Some(completed.clone()))
}

#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvision {
    pub boundary: BoundaryMachineId,
    pub realization: target_operations::BoundarySettlementRealization,
}

#[derive(Debug, Clone)]
struct RuntimeHostRequirement<A> {
    boundary: BoundaryMachineId,
    boundary_identity: String,
    selected_requirement_identity: String,
    source_contract: [u8; 32],
    binding: target_operations::NormalizedForeignCallBinding,
    calling_application: Option<calling_conventions::BoundaryCallingApplication>,
    native_adapter: Option<Result<A, String>>,
}

/// Native receipts and calling applications already supplied to admission.
/// Runtime capture rejoins them to the same canonical boundary before keeping
/// an adapter; no nominal provider report is invented for a Build binding.
#[derive(Clone, Copy)]
pub struct NativeBindingRuntimeInputs<'a> {
    pub bindings: &'a native_binding_admission::RetainedNativeBindings,
    pub calling: &'a [BoundaryCallingApplication],
}

#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvisions<A> {
    selection_identities: terminal_psi::TerminalIdentityTable,
    installation: Option<psi_provider_selections::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    host_requirements: Vec<RuntimeHostRequirement<A>>,
    borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts,
}

impl TerminalRuntimeHostProvisions<AdmittedRuntimeForeignCall> {
    pub fn admit(
        installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
        mut host: Vec<TerminalRuntimeHostProvision>,
        input: AdmittedRuntimeInput<'_>,
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
        external_bindings: &[calling_conventions::ExternalBindingRow],
        native: NativeBindingRuntimeInputs<'_>,
    ) -> Result<Self, String> {
        let module = input.module();
        let boundaries = input.boundary_machines();
        let identities = &module.identities;
        let mut host_requirements = Vec::new();
        for provision in &mut host {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let mut matching = boundaries
                .iter()
                .filter(|boundary| boundary.id == provision.boundary);
            let boundary = matching
                .next()
                .ok_or("runtime host provision lost its exact boundary declaration")?;
            if matching.next().is_some() {
                return Err("runtime host provision repeats its boundary declaration".into());
            }
            if let target_operations::NormalizedForeignCallMechanism::CompilerImport {
                import,
                requirement_identity,
                selected_plan,
            } = &binding.mechanism
            {
                // Psi ends its simulated process. Only native execution calls the operating system.
                if input.target_context().map(|context| context.target())
                    != Some(import.locator().target().target_name())
                    || selected
                        .selected_requirement_row(requirement_identity)
                        .is_none_or(|(plan, _)| plan.identity_digest().as_bytes() != selected_plan)
                    || identities.text(boundary.identity) != Some(requirement_identity.as_str())
                    || !binding.has_valid_entry_mechanism()
                    || terminal_semantics::closed_conformances::hosted_primitive_requirement(
                        module, boundary,
                    )?
                    .is_none()
                {
                    return Err(
                        "runtime process exit does not match the checked primitive and target"
                            .into(),
                    );
                }
                provision.realization = match import {
                    target_operations::CompilerImport::WindowsProcessExit
                    | target_operations::CompilerImport::WindowsArm64ProcessExit => {
                        target_operations::BoundarySettlementRealization::Builtin(
                            target_operations::HostedExitProcessI32Realization.into(),
                        )
                    }
                };
                continue;
            }
            let abi = module.calling_abi_identity(provision.boundary)?;
            let mut native_rows = native
                .bindings
                .rows
                .iter()
                .filter(|row| row.boundary == boundary.id);
            let native_row = native_rows.next();
            if native_rows.next().is_some() {
                return Err("runtime host provision has competing native bindings".into());
            }
            let calling_application = if let Some(row) = native_row {
                let profile = input
                    .target_context()
                    .ok_or("runtime native binding has no selected target")?;
                let profile = target::TargetProfile::from_canonical_target_name(profile.target())
                    .map_err(|error| error.to_string())?;
                let mut applications = native
                    .calling
                    .iter()
                    .filter(|application| application.abi == abi);
                let application = applications
                    .next()
                    .filter(|_| applications.next().is_none())
                    .ok_or("runtime native binding lost its unique calling application")?;
                let execution = native_binding_admission::validate_native_binding_execution(
                    row,
                    native.bindings,
                    module,
                    profile,
                    application,
                )?;
                if !binding.matches_execution(
                    target_operations::ExternalExecutionBinding::NativeBinding(execution),
                ) {
                    return Err(
                        "runtime native binding changed its admitted execution receipt".into(),
                    );
                }
                Some(application.clone())
            } else {
                let requirement = binding
                    .requirement_identity(selected.identities())
                    .ok_or("runtime foreign calling application lost its selected requirement")?;
                if !selected.selected_row_matches_requirement(
                    identities.text(boundary.identity).unwrap_or_default(),
                    requirement,
                    binding.provider_plan_report_identity(),
                ) {
                    return Err(
                        "runtime host provision lost its exact selected requirement row".into(),
                    );
                }
                let mut rows = external_bindings.iter().filter(|row| {
                    selected.selected_row_matches_requirement(
                        &row.requirement_identity,
                        requirement,
                        binding.provider_plan_report_identity(),
                    )
                });
                let row = rows.next();
                if rows.next().is_some() {
                    return Err("runtime foreign calling application is ambiguous".into());
                }
                row.and_then(|row| row.calling_application(&abi)).cloned()
            };
            if let Some(application) = &calling_application {
                if Some(application.target) != binding.native_target()
                    || application.plan.plan() != &binding.boundary_entry_plan
                    || application.commitment == [0; 32]
                    || application.recomputed_commitment()? != application.commitment
                {
                    return Err(
                        "runtime foreign calling application differs from its admitted row".into(),
                    );
                }
            } else if binding.foreign_import_locator().is_some() {
                return Err(
                    "runtime foreign import lost its complete admitted calling application".into(),
                );
            }
            let native_adapter = binding.foreign_import_locator().map(|locator| {
                let application = calling_application.as_ref().ok_or_else(|| {
                    "foreign runtime adapter has no retained complete calling application"
                        .to_owned()
                })?;
                AdmittedRuntimeForeignCall::from_admission(boundary, application, locator)
            });
            host_requirements.push(RuntimeHostRequirement {
                boundary: boundary.id,
                boundary_identity: identities
                    .text(boundary.identity)
                    .ok_or("runtime host provision lost its canonical requirement identity")?
                    .to_owned(),
                selected_requirement_identity: module
                    .boundary_machine(boundary.id)
                    .and_then(|boundary| boundary.declaration.as_ref())
                    .and_then(|declaration| identities.text(declaration.source.callable))
                    .ok_or("runtime host provision lost its declared requirement identity")?
                    .to_owned(),
                source_contract: boundary
                    .declaration
                    .as_ref()
                    .ok_or("runtime host provision lost its checked contract")?
                    .source
                    .contract,
                binding: binding.clone(),
                calling_application,
                native_adapter,
            });
        }
        Ok(Self {
            selection_identities: selected.identities().clone(),
            installation: installation.cloned(),
            host,
            host_requirements,
            borrowed_storage_layouts: input.borrowed_storage().clone(),
        })
    }
}

impl<A> TerminalRuntimeHostProvisions<A> {
    /// Transform only the owned auxiliary products. Stored refusals stay attached
    /// to their boundary: an unused unsupported adapter is not a root refusal.
    pub fn map_adapters<B>(
        self,
        mut transform: impl FnMut(A) -> Result<B, String>,
    ) -> TerminalRuntimeHostProvisions<B> {
        TerminalRuntimeHostProvisions {
            selection_identities: self.selection_identities,
            installation: self.installation,
            host: self.host,
            host_requirements: self
                .host_requirements
                .into_iter()
                .map(|row| RuntimeHostRequirement {
                    boundary: row.boundary,
                    boundary_identity: row.boundary_identity,
                    selected_requirement_identity: row.selected_requirement_identity,
                    source_contract: row.source_contract,
                    binding: row.binding,
                    calling_application: row.calling_application,
                    native_adapter: row
                        .native_adapter
                        .map(|adapter| adapter.and_then(&mut transform)),
                })
                .collect(),
            borrowed_storage_layouts: self.borrowed_storage_layouts,
        }
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        binding: &target_operations::NormalizedForeignCallBinding,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        let mut matching = self
            .host_requirements
            .iter()
            .filter(|row| row.boundary == declaration.id);
        matching.next().is_some_and(|row| {
            identities.text(declaration.identity) == Some(row.boundary_identity.as_str())
                && declaration
                    .declaration
                    .as_ref()
                    .and_then(|declaration| identities.text(declaration.source.callable))
                    == Some(row.selected_requirement_identity.as_str())
                && declaration
                    .declaration
                    .as_ref()
                    .map(|declaration| declaration.source.contract)
                    == Some(row.source_contract)
                && &row.binding == binding
        }) && matching.next().is_none()
    }

    pub fn host_calling_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&calling_conventions::BoundaryCallingApplication> {
        self.host_requirements
            .iter()
            .find(|row| row.boundary == boundary)?
            .calling_application
            .as_ref()
    }

    pub const fn installation(
        &self,
    ) -> Option<&psi_provider_selections::AdmittedProviderSelections> {
        self.installation.as_ref()
    }
    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selection_identities
    }
    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        &self.host
    }
    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        &self.borrowed_storage_layouts
    }

    pub fn require_borrowed_storage_layouts(
        &self,
        candidate: &runtime_abi::NativeBorrowedStorageLayouts,
    ) -> Result<(), &'static str> {
        if candidate != &self.borrowed_storage_layouts {
            return Err("borrowed storage geometry differs from the admitted runtime input");
        }
        Ok(())
    }
}

impl<T> TerminalRuntimeHostProvisions<Arc<T>> {
    pub fn retained_foreign_adapters(&self) -> impl Iterator<Item = &Arc<T>> {
        self.host_requirements
            .iter()
            .filter_map(|row| row.native_adapter.as_ref()?.as_ref().ok())
    }

    pub fn host_native_foreign_adapter(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<Result<&T, &str>> {
        self.host_requirements
            .iter()
            .find(|row| row.boundary == boundary)?
            .native_adapter
            .as_ref()
            .map(|adapter| adapter.as_ref().map(Arc::as_ref).map_err(String::as_str))
    }
}

#[derive(Debug, Clone)]
pub struct TerminalCallableRuntimeProvisions<A> {
    identity: TerminalPsiIdentity,
    manifest: [u8; 32],
    entry: MachineId,
    target: target::TargetProfile,
    host: TerminalRuntimeHostProvisions<A>,
    scalar_applications: Option<AdmittedScalarApplications>,
}

impl<A> TerminalCallableRuntimeProvisions<A> {
    pub fn map_adapters<B>(
        self,
        transform: impl FnMut(A) -> Result<B, String>,
    ) -> TerminalCallableRuntimeProvisions<B> {
        TerminalCallableRuntimeProvisions {
            identity: self.identity,
            manifest: self.manifest,
            entry: self.entry,
            target: self.target,
            host: self.host.map_adapters(transform),
            scalar_applications: self.scalar_applications,
        }
    }

    pub fn admit(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        input: AdmittedRuntimeInput<'_>,
        entry: MachineId,
        target: target::TargetProfile,
        host: TerminalRuntimeHostProvisions<A>,
    ) -> Result<Self, &'static str> {
        if artifact.module().machine(entry).is_none()
            || input.entry() != entry
            || host
                .installation()
                .is_some_and(|selection| selection.terminal_psi() != artifact.identity())
        {
            return Err("callable runtime provisions name another artifact or absent entry");
        }
        let scalar_applications =
            capture_scalar_applications(input, artifact.identity(), target, host.installation())?;
        host.require_borrowed_storage_layouts(input.borrowed_storage())?;
        Ok(Self {
            identity: artifact.identity(),
            manifest: input.manifest_identity(),
            entry,
            target,
            host,
            scalar_applications,
        })
    }
    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }

    pub fn matches_artifact(&self, artifact: &terminal_codec::CanonicalTerminalArtifact) -> bool {
        self.identity == artifact.view().identity()
            && &self.manifest == artifact.manifest().identity().as_bytes()
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub const fn target(&self) -> target::TargetProfile {
        self.target
    }
    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions<A> {
        &self.host
    }

    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct NativeRuntimePlan<A> {
    identity: TerminalPsiIdentity,
    manifest: [u8; 32],
    entry: MachineId,
    native_entry: MachineId,
    selected_providers: [u8; 32],
    source_providers: [u8; 32],
    source_native_bindings: [u8; 32],
    source_signature_identity: [u8; 32],
    target_slot: target::ProgramEntrySlotDeclaration,
    target_provider_proposals: Vec<TerminalTargetProviderProposal>,
    host_provisions: TerminalRuntimeHostProvisions<A>,
    private_entry_ram: Vec<CheckedPrivateEntryRamRecipe>,
    scalar_applications: Option<AdmittedScalarApplications>,
}

impl<A> NativeRuntimePlan<A> {
    pub fn map_adapters<B>(
        self,
        transform: impl FnMut(A) -> Result<B, String>,
    ) -> NativeRuntimePlan<B> {
        NativeRuntimePlan {
            identity: self.identity,
            manifest: self.manifest,
            entry: self.entry,
            native_entry: self.native_entry,
            selected_providers: self.selected_providers,
            source_providers: self.source_providers,
            source_native_bindings: self.source_native_bindings,
            source_signature_identity: self.source_signature_identity,
            target_slot: self.target_slot,
            target_provider_proposals: self.target_provider_proposals,
            host_provisions: self.host_provisions.map_adapters(transform),
            private_entry_ram: self.private_entry_ram,
            scalar_applications: self.scalar_applications,
        }
    }

    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }

    pub fn private_entry_ram(&self) -> &[CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }
    pub fn with_private_entry_ram(mut self, recipes: Vec<CheckedPrivateEntryRamRecipe>) -> Self {
        self.private_entry_ram = recipes;
        self
    }

    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions<A> {
        &self.host_provisions
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        binding: &target_operations::NormalizedForeignCallBinding,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        self.host_provisions
            .matches_host_requirement(declaration, binding, identities)
    }

    pub fn host_calling_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&calling_conventions::BoundaryCallingApplication> {
        self.host_provisions.host_calling_application(boundary)
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }

    pub fn matches_artifact(&self, artifact: &terminal_codec::CanonicalTerminalArtifact) -> bool {
        self.identity == artifact.view().identity()
            && &self.manifest == artifact.manifest().identity().as_bytes()
    }

    /// A physical adapter can have a different machine ID from the semantic entry.
    pub fn matches_retained_program(&self, program: &crate::RetainedTerminalArtifact) -> bool {
        let proposal = program.native_realization_proposal();
        let Some(entry) = proposal.program_entry() else {
            return false;
        };
        let checked = entry.checked_entry();
        let physical = entry
            .physical_adapter()
            .map_or(checked.terminal_entry(), |adapter| adapter.machine());
        self.matches_artifact(program.artifact())
            && self.entry == checked.terminal_entry()
            && self.native_entry == physical
            && self.source_signature_identity == checked.source_signature_identity()
            && self.target_slot == entry.source().target_slot()
            && self.target_slot.owner == proposal.target_profile()
            && &self.source_providers == proposal.providers().selection.identity_digest().as_bytes()
            && self.source_native_bindings == proposal.providers().native_bindings.identity_digest()
    }

    pub fn matches_image(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        native_entry: MachineId,
        profile: target::TargetProfile,
        selected_providers: &[u8; 32],
    ) -> bool {
        self.matches_artifact(artifact)
            && self.native_entry == native_entry
            && self.target_slot.owner == profile
            && &self.selected_providers == selected_providers
    }

    pub fn with_target_provider_proposals(
        mut self,
        proposals: Vec<TerminalTargetProviderProposal>,
    ) -> Self {
        self.target_provider_proposals = proposals;
        self
    }

    pub fn target_provider_proposals(&self) -> &[TerminalTargetProviderProposal] {
        &self.target_provider_proposals
    }

    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        self.host_provisions.borrowed_storage_layouts()
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        self.host_provisions.selection_identities()
    }

    pub const fn installation(
        &self,
    ) -> Option<&psi_provider_selections::AdmittedProviderSelections> {
        self.host_provisions.installation()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        self.host_provisions.host()
    }
}

impl NativeRuntimePlan<AdmittedRuntimeForeignCall> {
    pub fn admit(
        program_entry: NativeProgramEntrySettlement<'_>,
        input: AdmittedRuntimeInput<'_>,
        installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
        host: Vec<TerminalRuntimeHostProvision>,
        providers: &crate::ScopedProviderSelection,
        external_bindings: &[calling_conventions::ExternalBindingRow],
        native: NativeBindingRuntimeInputs<'_>,
    ) -> Result<Self, String> {
        let checked = program_entry
            .checked_entry
            .ok_or("runtime provisions have no checked semantic entry")?;
        let target_slot = program_entry.source().target_slot();
        let native_entry = program_entry
            .physical_adapter()
            .map_or(checked.terminal_entry(), |adapter| adapter.machine());
        let scope = input.require_native_providers(providers, native.bindings)?;
        let selected = providers.selection();
        if scope.checked_entry() != Some(checked)
            || scope.reach().entry() != native_entry
            || scope.reach().target_context().target() != target_slot.owner.target_name()
            || scope.reach().target_context().terminal_psi() != checked.terminal_psi_identity()
            || input.entry() != native_entry
            || checked.source_signature_identity() != program_entry.source().identity().bytes()
            || program_entry
                .physical_adapter()
                .is_some_and(|adapter| adapter.terminal() != checked.terminal_psi_identity())
        {
            return Err("runtime provisions name another admitted entry".into());
        }
        let scalar_applications = capture_scalar_applications(
            input,
            checked.terminal_psi_identity(),
            target_slot.owner,
            installation,
        )?;
        let host_provisions = TerminalRuntimeHostProvisions::admit(
            installation,
            host,
            input,
            selected,
            external_bindings,
            native,
        )?;
        Ok(Self {
            private_entry_ram: Vec::new(),
            scalar_applications,
            identity: checked.terminal_psi_identity(),
            manifest: input.manifest_identity(),
            entry: checked.terminal_entry(),
            native_entry,
            selected_providers: scope.selected_provider_closure(),
            source_providers: *scope
                .source_provider_selection()
                .identity_digest()
                .as_bytes(),
            source_native_bindings: scope.source_native_bindings(),
            source_signature_identity: checked.source_signature_identity(),
            target_slot,
            target_provider_proposals: Vec::new(),
            host_provisions,
        })
    }
}
