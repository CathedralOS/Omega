/*
We capture source-free runtime provisions (`TerminalRuntimeHostProvisions`,
`TerminalCallableRuntimeProvisions`, and `TerminalRuntimeProvisions`) from
admitted native compilation inputs before image emission consumes them. We bind
host boundary settlements to their original requirement identities, calling
applications, and fixed-frame foreign adapter images (`emit_native_foreign_adapter`),
and we validate component product rosters and adjacent publication receipts
against root object imports.
*/

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarType};
use std::sync::Arc;
use target::NormalizedImportLocator;
use terminal_installation::AdmittedScalarApplications;
use terminal_installation::CheckedPrivateEntryRamRecipe;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind, TerminalPsiIdentity};
use terminal_psi_to_abstract_operations::TerminalTargetProviderProposal;

fn scalar_parameters(
    declaration: &BoundaryMachineDeclaration,
    application: &BoundaryCallingApplication,
) -> Result<Vec<Option<ScalarType>>, String> {
    if !declaration.has_valid_parameter_order() {
        return Err("foreign frame scalar transport has no complete semantic formal order".into());
    }
    let formals = declaration
        .parameter_positions(BoundaryParameterKind::Scalar)
        .zip(&declaration.scalar_parameters)
        .collect::<Vec<_>>();
    let rows = application
        .signature
        .native_parameters
        .span(application.signature.native_parameter_rows)
        .ok_or("foreign frame scalar transport has stale native telescope storage")?;
    let count = application.plan.plan().call.parameters.len();
    if rows.len() != count {
        return Err("foreign frame scalar transport has an incomplete native telescope".into());
    }
    let mut result = vec![None; count];
    let mut native_seen = vec![false; count];
    let mut scalar_seen = vec![false; formals.len()];
    for row in rows {
        let ordinal = usize::try_from(row.native_ordinal)
            .map_err(|_| "foreign frame native ordinal overflows")?;
        let seen = native_seen
            .get_mut(ordinal)
            .ok_or("foreign frame native ordinal is outside its calling plan")?;
        if std::mem::replace(seen, true) {
            return Err("foreign frame native ordinal is repeated".into());
        }
        let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } = row.origin else {
            continue;
        };
        let formal = usize::try_from(formal_ordinal)
            .map_err(|_| "foreign frame semantic ordinal overflows")?;
        if formal >= declaration.parameter_order.len() {
            return Err("foreign frame semantic ordinal is outside its declaration".into());
        }
        if let Some((index, (_, scalar))) = formals
            .iter()
            .enumerate()
            .find(|(_, (position, _))| *position == formal)
        {
            if std::mem::replace(&mut scalar_seen[index], true) {
                return Err("foreign frame scalar formal has repeated native transport".into());
            }
            result[ordinal] = Some(**scalar);
        }
    }
    if scalar_seen.contains(&false) {
        return Err("foreign frame lost a scalar formal's native transport".into());
    }
    Ok(result)
}

fn emit_native_adapter(
    declaration: &BoundaryMachineDeclaration,
    application: &BoundaryCallingApplication,
    locator: &target::NormalizedForeignLocator,
) -> Result<Arc<image_emission::NativeForeignAdapterImage>, String> {
    let scalar_parameters = scalar_parameters(declaration, application)?;
    let bridge = machine_emission::native_foreign_bridge::emit_native_foreign_bridge(
        machine_emission::native_foreign_bridge::NativeForeignBridgeRequest {
            target: application.target,
            foreign: application.plan.clone(),
            scalar_parameters,
        },
    )
    .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))?;
    image_emission::emit_native_foreign_adapter(bridge, locator.clone(), application.commitment)
        .map(Arc::new)
        .map_err(|error| error.diagnostic.to_string())
}

fn capture_scalar_applications(
    input: &terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    identity: TerminalPsiIdentity,
    target: target::TargetProfile,
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
) -> Result<Option<AdmittedScalarApplications>, &'static str> {
    if input.plan().psi != identity
        || input.target_machine_selections().is_none_or(|selected| {
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
struct RuntimeHostRequirement {
    boundary: BoundaryMachineId,
    boundary_identity: String,
    selected_requirement_identity: String,
    provider_plan_report_identity: u64,
    calling_application: Option<calling_conventions::BoundaryCallingApplication>,
    native_adapter: Option<Result<Arc<image_emission::NativeForeignAdapterImage>, String>>,
}

#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvisions {
    selection_identities: terminal_psi::TerminalIdentityTable,
    installation: Option<terminal_installation::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    host_requirements: Vec<RuntimeHostRequirement>,
    borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

impl TerminalRuntimeHostProvisions {
    pub fn capture_native_admission(
        installation: Option<&terminal_installation::AdmittedProviderSelections>,
        host: Vec<TerminalRuntimeHostProvision>,
        input: &terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
        selected: &effects::SelectedProviderPlanFacts,
        external_bindings: &[calling_conventions::ExternalBindingRow],
    ) -> Result<Self, &'static str> {
        let boundaries = &input.plan().boundary_machines;
        let module = input.module();
        let identities = &module.identities;
        let mut host_requirements = Vec::new();
        for provision in &host {
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
            if matching.next().is_some()
                || !selected.selected_row_matches_requirement(
                    identities.text(boundary.identity).unwrap_or_default(),
                    binding
                        .requirement_identity(selected.identities())
                        .unwrap_or_default(),
                    binding.provider_plan_report_identity(),
                )
            {
                return Err("runtime host provision lost its exact selected requirement row");
            }
            let requirement = binding
                .requirement_identity(selected.identities())
                .ok_or("runtime foreign calling application lost its selected requirement")?;
            let mut rows = external_bindings.iter().filter(|row| {
                selected.selected_row_matches_requirement(
                    &row.requirement_identity,
                    requirement,
                    binding.provider_plan_report_identity(),
                )
            });
            let row = rows.next();
            if rows.next().is_some() {
                return Err("runtime foreign calling application is ambiguous");
            }
            let abi = input
                .optimization_input()
                .selected_calling_abi_identity(provision.boundary)
                .map_err(|_| "runtime host provision lost its exact declaration ABI")?;
            let calling_application = row
                .and_then(|row| row.calling_application(&abi))
                .map(|application| {
                    if Some(application.target) != binding.native_target()
                        || application.plan.plan() != &binding.boundary_entry_plan
                        || application.commitment == [0; 32]
                        || application.recomputed_commitment() != Ok(application.commitment)
                    {
                        return Err(
                            "runtime foreign calling application differs from its admitted row",
                        );
                    }
                    Ok(application.clone())
                })
                .transpose()?;
            if calling_application.is_none() && binding.import().is_some() {
                return Err(
                    "runtime foreign import lost its complete admitted calling application",
                );
            }
            let native_adapter = binding.import().map(|(locator, _)| {
                let application = calling_application.as_ref().ok_or_else(|| {
                    "foreign runtime adapter has no retained complete calling application"
                        .to_owned()
                })?;
                emit_native_adapter(boundary, application, locator)
            });
            host_requirements.push(RuntimeHostRequirement {
                boundary: boundary.id,
                boundary_identity: identities
                    .text(boundary.identity)
                    .unwrap_or_default()
                    .to_owned(),
                selected_requirement_identity: binding
                    .requirement_identity(selected.identities())
                    .unwrap_or_default()
                    .to_owned(),
                provider_plan_report_identity: binding.provider_plan_report_identity(),
                calling_application,
                native_adapter,
            });
        }
        Ok(Self {
            selection_identities: selected.identities().clone(),
            installation: installation.cloned(),
            host,
            host_requirements,
            borrowed_storage_layouts: Vec::new(),
        })
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        selected_requirement: &str,
        provider_plan_report_identity: u64,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        let mut matching = self
            .host_requirements
            .iter()
            .filter(|row| row.boundary == declaration.id);
        matching.next().is_some_and(|row| {
            identities.text(declaration.identity) == Some(row.boundary_identity.as_str())
                && row.selected_requirement_identity == selected_requirement
                && row.provider_plan_report_identity == provider_plan_report_identity
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

    pub fn host_native_foreign_adapter(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<Result<&image_emission::NativeForeignAdapterImage, &str>> {
        self.host_requirements
            .iter()
            .find(|row| row.boundary == boundary)?
            .native_adapter
            .as_ref()
            .map(|adapter| adapter.as_ref().map(Arc::as_ref).map_err(String::as_str))
    }

    pub const fn installation(&self) -> Option<&terminal_installation::AdmittedProviderSelections> {
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
    pub fn with_borrowed_storage_layouts(
        mut self,
        layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    ) -> Self {
        self.borrowed_storage_layouts = layouts;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TerminalCallableRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    target: target::TargetProfile,
    host: TerminalRuntimeHostProvisions,
    scalar_applications: Option<AdmittedScalarApplications>,
}

impl TerminalCallableRuntimeProvisions {
    pub fn capture_native_admission(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        input: &terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
        entry: MachineId,
        target: target::TargetProfile,
        host: TerminalRuntimeHostProvisions,
    ) -> Result<Self, &'static str> {
        if artifact.module().machine(entry).is_none()
            || input.plan().entry != entry
            || host
                .installation()
                .is_some_and(|selection| selection.terminal_psi() != artifact.identity())
        {
            return Err("callable runtime provisions name another artifact or absent entry");
        }
        let scalar_applications =
            capture_scalar_applications(input, artifact.identity(), target, host.installation())?;
        Ok(Self {
            identity: artifact.identity(),
            entry,
            target,
            host,
            scalar_applications,
        })
    }
    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }
    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub const fn target(&self) -> target::TargetProfile {
        self.target
    }
    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions {
        &self.host
    }

    pub fn with_borrowed_storage_layouts(
        mut self,
        layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    ) -> Self {
        self.host = self.host.with_borrowed_storage_layouts(layouts);
        self
    }

    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct TerminalRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    source_signature_identity: [u8; 32],
    target_slot: target::ProgramEntrySlotDeclaration,
    target_provider_proposals: Vec<TerminalTargetProviderProposal>,
    host_provisions: TerminalRuntimeHostProvisions,
    private_entry_ram: Vec<CheckedPrivateEntryRamRecipe>,
    components: Vec<Arc<crate::NativeComponentProduct>>,
    component_publications: Vec<crate::NativeComponentPublicationReceipt>,
    scalar_applications: Option<AdmittedScalarApplications>,
}

impl TerminalRuntimeProvisions {
    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }

    pub fn record_component_publications(
        &mut self,
        receipts: Vec<crate::NativeComponentPublicationReceipt>,
    ) -> Result<(), &'static str> {
        if !self.component_publications.is_empty() || receipts.len() != self.components.len() {
            return Err(
                "component publication must cover the current immutable product roster exactly once",
            );
        }
        for (receipt, component) in receipts.iter().zip(&self.components) {
            if receipt.product().locator() != component.locator()
                || receipt.product().selection() != component.selection()
                || receipt.product().application() != component.application()
                || receipt.product().artifact().manifest().identity()
                    != component.artifact().manifest().identity()
                || receipt.product().authority().review().identity()
                    != component.authority().review().identity()
            {
                return Err(
                    "component publication receipt belongs to another exact producing image or historical input graph",
                );
            }
        }
        self.component_publications = receipts;
        Ok(())
    }

    pub fn validate_component_process_inputs(&self, root: &std::path::Path) -> Result<(), String> {
        self.validate_component_product_roster()
            .map_err(str::to_owned)?;
        if self.components.is_empty() {
            return Ok(());
        }
        if self.component_publications.len() != self.components.len() {
            return Err(
                "component process has no complete exact publication receipt roster".into(),
            );
        }
        let directory = root
            .parent()
            .ok_or("component process root has no publication directory")?;
        for receipt in &self.component_publications {
            if receipt.path().parent() != Some(directory) {
                return Err("component process dependency is outside the selected root's adjacent immutable publication graph".into());
            }
            receipt.revalidate()?;
        }
        Ok(())
    }
    pub fn component_products(&self) -> &[Arc<crate::NativeComponentProduct>] {
        &self.components
    }
    pub fn with_component_products(
        mut self,
        components: Vec<Arc<crate::NativeComponentProduct>>,
    ) -> Result<Self, &'static str> {
        self.validate_component_products(&components)?;
        self.components = components;
        Ok(self)
    }

    pub fn validate_root_object(
        object: &image_emission::ObjectArtifact,
        runtime: Option<&Self>,
    ) -> Result<(), &'static str> {
        match runtime {
            Some(runtime) => runtime.validate_component_root_object(object),
            None if object
                .object()
                .layout
                .normalized_imports
                .iter()
                .any(|import| matches!(import.locator, NormalizedImportLocator::Component(_))) =>
            {
                Err(
                    "component native product has no complete retained runtime image/resource roster",
                )
            }
            None => Ok(()),
        }
    }

    pub fn validate_component_root_object(
        &self,
        object: &image_emission::ObjectArtifact,
    ) -> Result<(), &'static str> {
        if installation_evidence::ObjectEvidence::psi(object) != self.identity
            || object.target() != self.target_slot.owner.native_target()
        {
            return Err("component runtime provisions name another actual root artifact or target");
        }
        self.validate_component_product_roster()?;
        for import in &object.object().layout.normalized_imports {
            let NormalizedImportLocator::Component(locator) = &import.locator else {
                continue;
            };
            let mut products = self
                .components
                .iter()
                .filter(|product| product.locator() == locator);
            if products.next().is_none() || products.next().is_some() {
                return Err(
                    "actual component root import has no unique exact retained image/resource product",
                );
            }
        }
        for call in object.foreign_calls() {
            if !matches!(
                &call.binding.mechanism,
                target_operations::NormalizedForeignCallMechanism::Component { .. }
            ) {
                continue;
            }
            if !self.host().iter().any(|provision| {
                matches!(&provision.realization,
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                        if binding == &call.binding)
            }) {
                return Err(
                    "actual component root call differs from its complete runtime ABI, provider or same-stack settlement",
                );
            }
        }
        Ok(())
    }

    pub fn validate_component_product_roster(&self) -> Result<(), &'static str> {
        self.validate_component_products(&self.components)
    }
    fn validate_component_products(
        &self,
        components: &[Arc<crate::NativeComponentProduct>],
    ) -> Result<(), &'static str> {
        let mut expected = 0;
        for provision in self.host() {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let target_operations::NormalizedForeignCallMechanism::Component {
                locator,
                provider,
                candidate,
                application_commitment,
                same_stack_contribution,
            } = &binding.mechanism
            else {
                continue;
            };
            expected += 1;
            let mut matches = components.iter().filter(|component| {
                component.selection().receiver()
                    == &terminal_codec::TerminalExecutionIdentity::portable(self.identity)
                    && component.selection().receiving_boundary() == provision.boundary
                    && component.selection().provider() == *provider
                    && component.selection().candidate() == *candidate
                    && component.locator() == locator
                    && component.application().commitment == *application_commitment
                    && component.application().plan.plan() == &binding.boundary_entry_plan
            });
            let component = matches.next().ok_or(
                "component runtime roster does not exactly cover an admitted independent native call",
            )?;
            if matches.next().is_some() {
                return Err(
                    "component runtime roster ambiguously duplicates an admitted independent native call",
                );
            }
            component.validate_same_stack(same_stack_contribution)?;
        }
        if expected != components.len() {
            return Err("component runtime roster contains an unconsumed or duplicated image");
        }
        Ok(())
    }
    pub fn private_entry_ram(&self) -> &[CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }
    pub fn with_private_entry_ram(mut self, recipes: Vec<CheckedPrivateEntryRamRecipe>) -> Self {
        self.private_entry_ram = recipes;
        self
    }

    pub fn capture_native_admission(
        program_entry: terminal_psi_to_abstract_operations::NativeProgramEntrySettlement<'_>,
        input: &terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
        installation: Option<&terminal_installation::AdmittedProviderSelections>,
        host: Vec<TerminalRuntimeHostProvision>,
        selected: &effects::SelectedProviderPlanFacts,
        external_bindings: &[calling_conventions::ExternalBindingRow],
    ) -> Result<Self, &'static str> {
        let checked = program_entry
            .checked_entry
            .ok_or("runtime provisions have no checked semantic entry")?;
        let target_slot = program_entry.source().target_slot();
        let native_entry = program_entry
            .physical_adapter()
            .map_or(checked.terminal_entry(), |adapter| adapter.machine());
        if input.plan().entry != native_entry
            || checked.source_signature_identity() != program_entry.source().identity().bytes()
            || program_entry
                .physical_adapter()
                .is_some_and(|adapter| adapter.terminal() != checked.terminal_psi_identity())
        {
            return Err("runtime provisions name another admitted entry");
        }
        let scalar_applications = capture_scalar_applications(
            input,
            checked.terminal_psi_identity(),
            target_slot.owner,
            installation,
        )?;
        let host_provisions = TerminalRuntimeHostProvisions::capture_native_admission(
            installation,
            host,
            input,
            selected,
            external_bindings,
        )?;
        Ok(Self {
            private_entry_ram: Vec::new(),
            components: Vec::new(),
            component_publications: Vec::new(),
            scalar_applications,
            identity: checked.terminal_psi_identity(),
            entry: checked.terminal_entry(),
            source_signature_identity: checked.source_signature_identity(),
            target_slot,
            target_provider_proposals: Vec::new(),
            host_provisions,
        })
    }

    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions {
        &self.host_provisions
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        selected_requirement: &str,
        provider_plan_report_identity: u64,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        self.host_provisions.matches_host_requirement(
            declaration,
            selected_requirement,
            provider_plan_report_identity,
            identities,
        )
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

    pub fn with_borrowed_storage_layouts(
        mut self,
        layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    ) -> Self {
        self.host_provisions.borrowed_storage_layouts = layouts;
        self
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

    pub const fn installation(&self) -> Option<&terminal_installation::AdmittedProviderSelections> {
        self.host_provisions.installation()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        self.host_provisions.host()
    }
}
